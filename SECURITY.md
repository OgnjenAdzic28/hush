# Security policy

Hush reads and controls other apps on your machine on an agent's behalf, so bugs that let it act
outside the target app, leak what it reads, or bypass the OS permission prompts matter.

## Reporting a vulnerability

Please report it privately through GitHub: open the repository's **Security** tab and choose
**Report a vulnerability**. Don't open a public issue.

Include what you did, what happened and what you expected. You'll get a reply within a week.

## Scope

In scope: Hush's own code, including how it routes input, which process receives events, what it
returns to agents, and the MCP server.

Out of scope: vulnerabilities in the agents that call Hush, or in the OS itself. Report those to
their maintainers.
