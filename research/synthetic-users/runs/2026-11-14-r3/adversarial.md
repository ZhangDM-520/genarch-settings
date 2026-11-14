# round 3 — adversarial

- under test: commit `0371256` installed from the payload on `r3-adv` (Arch cloud, 2 vCPU)
- raw transcript: `adversarial-raw.txt` in this directory
- question: junk, empty, half-written, interrupted, removed and renamed inputs — does the engine
  stay loud, write nothing it cannot classify, and recover?

| # | attack | result |
|---|---|---|
| A1 | empty base (`usr/lib/genarch-settings` with no `intent.toml`) | exit **2**, `no intent at … — is the package installed?` |
| A2 | `sysctl.toml` with a bare `kernel.printk` key (no `[values]`) | exit **2**, serde names the field it expected (`values`); a key inside `[values]` that parses as a table gets the explicit "quote it" hint |
| A3 | unknown key in a layer's `intent.toml` | exit **2**, the message lists every accepted field |
| A4 | selection names a profile that does not exist | exit **2**, names the selection file and both search roots |
| A5 | `apply` as a non-root user on the live root | exit **3** (`nothing was written`, the pinned unclassifiable refusal); no partial write |
| A6 | `SIGKILL` mid-apply (26 files landed, no manifest) | `report` shows MIRROR/MANAGED/SYNC/CEIL drift; a re-apply adopts the files (`applied=40`, manifest present, `PLAN 0`) — the accepted write-ahead-manifest gap heals on the next run |
| A7 | `pacman -R`, then reinstall | removal prints the manifest/pre-image warning and leaves deployed files and pre-images; reinstall converges with the same manifest; the loud no-converge message is printed |
| A8 | stale `pid=999999` text in the lock file, no `flock` held | apply proceeds (the lock is the truth, not the text), `applied=55` |
| A9 | renamed binary (`/tmp/gs-renamed`) | `report` works (exit 1 = drift); an unknown verb is a clap error, not a panic |

No new defect: every attack either refuses loudly with exit 2/3 or recovers. Two observations held
as limitations, not issues: the A2 message is serde's field list rather than the explicit quoting
hint (that hint fires for a table *inside* `[values]`), and A6 leaves the landed files outside the
manifest until the next apply — the round-1 decision, now shown to heal.
