//! Text read from a window's pixels, for apps that draw their own interface.
//!
//! An app built on a GPU toolkit (egui, games, many editors) can show a full
//! interface while exposing almost nothing to Accessibility: FilmCraft, a
//! Premiere-style editor, lists a search field and a slider. Everything else
//! (panels, tabs, timecodes, clips) exists only as pixels. Apple's on-device
//! text recognition reads them: in fast mode, about 70 labels from a
//! 1600x980 window in 30 ms (macOS 27.2; accurate mode fails there with an
//! inference-engine error). Each label becomes a text element with its frame,
//! so the agent can click or drag it like any other element.
//!
//! The picture is read at twice the window's size in points: on a 1x display
//! (Hush's lane is one) small interface text is too few pixels tall for fast
//! recognition; doubling it took FilmCraft from 61 mostly misspelled labels
//! to 72 mostly right ones.

use std::hash::{DefaultHasher, Hash, Hasher};

use hush_core::{Error, Rect, Result};
use objc2::rc::Retained;
use objc2::{AnyThread, msg_send};
use objc2_core_foundation::CGRect;
use objc2_foundation::{NSArray, NSDictionary, NSError, NSRange};
use objc2_vision::{
    VNImageRequestHandler, VNRecognizeTextRequest, VNRecognizedText, VNRectangleObservation,
    VNRequest, VNRequestTextRecognitionLevel,
};

use crate::capture;

/// One label seen in the window.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Seen {
    pub(crate) text: String,
    /// Window-relative points.
    pub(crate) frame: Rect,
}

impl Seen {
    /// The same for the same label in the same place, so a label keeps its
    /// ref from one read to the next.
    pub(crate) fn id(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.text.hash(&mut hasher);
        ((self.frame.x / 8.0).round() as i64).hash(&mut hasher);
        ((self.frame.y / 8.0).round() as i64).hash(&mut hasher);
        hasher.finish()
    }
}

/// Labels this short are mostly icons misread as letters.
const MIN_CHARS: usize = 2;

/// The text in the window, in reading order (rows top to bottom, then left
/// to right).
pub(crate) fn read(window_id: u32, frame: Rect) -> Result<Vec<Seen>> {
    let (width, height) = ((frame.w * 2.0) as usize, (frame.h * 2.0) as usize);
    let found = capture::with_picture(window_id, width, height, |image| {
        let request = VNRecognizeTextRequest::new();
        request.setRecognitionLevel(VNRequestTextRecognitionLevel::Fast);
        request.setUsesLanguageCorrection(false);
        // SAFETY: a live image and an empty options dictionary.
        let handler = unsafe {
            VNImageRequestHandler::initWithCGImage_options(
                VNImageRequestHandler::alloc(),
                image,
                &NSDictionary::new(),
            )
        };
        let as_request: Retained<VNRequest> =
            Retained::into_super(Retained::into_super(request.clone()));
        handler
            .performRequests_error(&NSArray::from_retained_slice(&[as_request]))
            .map_err(|e| Error::Failed(format!("text recognition failed: {e}")))?;
        let to_points = |b: CGRect| Rect {
            // Normalized, with the origin at the bottom left.
            x: b.origin.x * frame.w,
            y: (1.0 - b.origin.y - b.size.height) * frame.h,
            w: b.size.width * frame.w,
            h: b.size.height * frame.h,
        };
        let mut seen = Vec::new();
        for observation in request.results().into_iter().flatten() {
            let Some(best) = observation.topCandidates(1).firstObject() else {
                continue;
            };
            // SAFETY: a plain getter on a live observation.
            let line = to_points(unsafe { observation.boundingBox() });
            let words: Vec<(String, Rect)> = words(&best)
                .into_iter()
                .map(|(word, b)| (word, to_points(b)))
                .collect();
            for (text, frame) in split_at_gaps(words, line) {
                if text.chars().filter(|c| c.is_alphanumeric()).count() >= MIN_CHARS {
                    seen.push(Seen { text, frame });
                }
            }
        }
        Ok(seen)
    })??;
    Ok(in_reading_order(found))
}

/// Each word of a recognized line with its normalized box. Vision returns
/// one line for labels that sit close together (the clips of a timeline),
/// and only the word boxes show where one label ends and the next begins.
fn words(text: &VNRecognizedText) -> Vec<(String, CGRect)> {
    let string = text.string().to_string();
    let mut out = Vec::new();
    let mut offset = 0usize; // in UTF-16 units, as NSRange counts
    for piece in string.split(' ') {
        let length = piece.encode_utf16().count();
        if !piece.is_empty() {
            let range = NSRange::new(offset, length);
            // SAFETY: `boundingBoxForRange:error:` takes an NSRange inside the
            // string and returns a rectangle observation or nil.
            let box_: Option<Retained<VNRectangleObservation>> = unsafe {
                msg_send![text, boundingBoxForRange: range, error: std::ptr::null_mut::<*mut NSError>()]
            };
            if let Some(b) = box_ {
                // SAFETY: a plain getter on a live observation.
                out.push((piece.to_owned(), unsafe { b.boundingBox() }));
            }
        }
        offset += length + 1;
    }
    out
}

/// Words grouped into labels: a gap wider than most of a line's height
/// between two words is a gap between two labels (a word space is about a
/// third of it).
fn split_at_gaps(words: Vec<(String, Rect)>, line: Rect) -> Vec<(String, Rect)> {
    if words.is_empty() {
        return vec![];
    }
    let mut labels: Vec<(String, Rect)> = Vec::new();
    for (word, frame) in words {
        match labels.last_mut() {
            Some((text, at)) if frame.x - (at.x + at.w) < line.h * 0.8 => {
                text.push(' ');
                text.push_str(&word);
                let right = (at.x + at.w).max(frame.x + frame.w);
                at.y = at.y.min(frame.y);
                at.h = at.h.max(frame.h);
                at.w = right - at.x;
            }
            _ => labels.push((word, frame)),
        }
    }
    labels
}

/// Rows are labels whose vertical centers are within half a line of each
/// other.
fn in_reading_order(mut seen: Vec<Seen>) -> Vec<Seen> {
    seen.sort_by(|a, b| a.frame.y.total_cmp(&b.frame.y));
    let mut rows: Vec<Vec<Seen>> = Vec::new();
    for label in seen {
        let center = label.frame.y + label.frame.h / 2.0;
        match rows.last_mut() {
            Some(row)
                if (row[0].frame.y + row[0].frame.h / 2.0 - center).abs()
                    < row[0].frame.h.max(label.frame.h) / 2.0 =>
            {
                row.push(label)
            }
            _ => rows.push(vec![label]),
        }
    }
    rows.into_iter()
        .flat_map(|mut row| {
            row.sort_by(|a, b| a.frame.x.total_cmp(&b.frame.x));
            row
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(text: &str, x: f64, y: f64) -> Seen {
        Seen {
            text: text.into(),
            frame: Rect {
                x,
                y,
                w: 40.0,
                h: 14.0,
            },
        }
    }

    #[test]
    fn labels_read_in_rows_left_to_right() {
        let order = in_reading_order(vec![
            at("Export", 280.0, 14.0),
            at("Clip", 20.0, 60.0),
            at("Import", 160.0, 16.0),
            at("Edit", 220.0, 12.0),
        ]);
        let texts: Vec<&str> = order.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(texts, ["Import", "Edit", "Export", "Clip"]);
    }

    #[test]
    fn wide_gaps_split_a_line_into_labels() {
        let word = |text: &str, x: f64, w: f64| {
            (
                text.to_owned(),
                Rect {
                    x,
                    y: 10.0,
                    w,
                    h: 12.0,
                },
            )
        };
        let line = Rect {
            x: 0.0,
            y: 10.0,
            w: 400.0,
            h: 12.0,
        };
        let labels = split_at_gaps(
            vec![
                word("Video", 0.0, 30.0),
                word("1", 34.0, 6.0),
                word("Ocean.mp4", 80.0, 60.0),
            ],
            line,
        );
        let texts: Vec<&str> = labels.iter().map(|(t, _)| t.as_str()).collect();
        assert_eq!(texts, ["Video 1", "Ocean.mp4"]);
        assert_eq!(labels[0].1.w, 40.0);
    }

    #[test]
    fn a_label_keeps_its_id_when_it_barely_moves() {
        assert_eq!(
            at("Video 1", 100.0, 200.0).id(),
            at("Video 1", 101.0, 201.0).id()
        );
        assert_ne!(
            at("Video 1", 100.0, 200.0).id(),
            at("Video 2", 100.0, 200.0).id()
        );
    }
}
