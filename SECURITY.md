# Security

`upm` runs other programs. Its job is to do that predictably and to refuse
ambiguous input.

## Model

**Trusted:** the package managers themselves and their configured repositories.
`upm` does not add repositories, download packages, or execute install scripts.

**Untrusted:** command-line arguments, package names and the user config file.

## Controls

| Threat | Control |
| ------ | ------- |
| Shell injection via package names | Commands are argv vectors; no shell is used |
| Option injection (`--force`, `-rf`) | Names starting with `-` are rejected |
| Control characters / whitespace | Rejected before execution |
| Accidental privilege escalation | `sudo` only on Unix, only for backends that need it, and the full command is printed first |
| Silent system changes | Interactive confirmation for mutating actions; `--dry-run` runs nothing |
| Misleading output | Diagnostics go to stderr, structured data to stdout |
| Config tampering | Config is read-only at runtime; defaults are written `0600` on Unix |
| Malicious alias overrides | Overrides are explicit user config; the resolved native name is shown |

## Non-goals

`upm` cannot protect you from a compromised package manager, a malicious
package, or a hostile repository. It intentionally delegates trust to the
native tool.

## Reporting

Open a private security advisory at
`https://github.com/zzzit0ti-cell/upm/security/advisories/new` rather than a
public issue. Include a reproduction and the output of `upm --version` and
`upm which --json`.
