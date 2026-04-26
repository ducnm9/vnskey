<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Bug Analysis (POL-10)

Root cause analyses for bugs discovered via the Bench compatibility matrix.

## Process

1. From the dashboard, pick the top 3 bugs by impact (most app x combo affected).
2. Create `<id>.md` in this directory with the template below.
3. Coordinate with upstream maintainer before writing a patch.
4. Submit PR to upstream, linking back to the analysis doc.

## Template

```markdown
# <BUG-ID>: Short description

## Affected combos

| Engine | App | Session | Accuracy drop |
|--------|-----|---------|---------------|

## Root cause

## Upstream issue

## Fix approach

## PR status
```
