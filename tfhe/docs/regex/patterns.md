# Supported regex patterns

This document specifies the supported set of regex patterns in the regex engine.

## Components

A regex is described by a sequence of components surrounded by `/`, the
following components are supported:

| Name | Notation | Examples |
| --- | --- |
| Character | Simply the character itself | `/a/`, `/b/`, `/Z/`, `/5/` |
| Character range | `[<character>-<character]` | `/[a-d]/`, `/[C-H]`/ |
| Any character | `.` | `/a.c/` |
| Escaped symbol | `\<symbol>` | `/\^/`, `/\$/` |
| Parenthesis | `(<regex>)` | `/(abc)*/`, `/d(ab)?/` |
| Optional | `<regex>?` | `/a?/`, `/(az)?/` |
| Zero or more | `<regex>*` | `/a*/`, `/ab*c/` |
| One or more | `<regex>+` | `/a+/`, `/ab+c/` |
| Exact repeat | `<regex{<number>}>` | `/ab{2}c/` |
| At least repeat | `<regex{<number>,}>` | `/ab{2,}c/` |
| At most repeat | `<regex{,<number>}>` | `/ab{,2}c/` |
| Repeat between | `<regex{<number>,<number>}>` | `/ab{2,4}c/` |
| Either | `<regex>|<regex>` | `/a\|b/`, `/ab\|cd/` |
| Start matching | `^<regex>` | `/^abc/` |
| End matching | `<regex>$` | `/abc$/` |

## Modifiers

Modifiers are mode selectors that affect the entire regex's behavior. At the
moment there is 1 modifier supported:

- Case insensitive matching, by appending a `i` after the regex pattern. For example: `/abc/i`

## General examples

These components and modifiers can be combined to form any desired regex
pattern. To give some idea of what's possible, here is a sequence of supported
regex patterns:
