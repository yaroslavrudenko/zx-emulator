#!/bin/sh
# Turn a folder of tapes into a shelf of cartridges.
#
#     sh tools/forge.sh ~/games
#
# ## What a cartridge is here, and why the word is not decoration
#
# A `.tap` is a cassette and it behaves like one: `zx-shot` types `LOAD ""`, presses PLAY, and the
# real ROM counts edges for **thousands of frames** — `docs/images/README.md` records the *R-Type*
# cassette at 28,259 of them, roughly nine minutes of emulated time — before anything playable is
# on the screen. That is faithful, and it is unusable as a way to start a game.
#
# A `.z80` of the machine at the end of that load is the same machine, and takes as long to arrive
# as it takes to read 40 KB. That is the difference between a cassette and a cartridge and it is
# the only difference: nothing is re-created, the state is carried. `zx-shot --snapshot` is the
# flag that writes it; this script is the loop that runs it once per game.
#
# ## What it writes, and where
#
# Into `cartridges/` at the repository root:
#
#   <slug>.z80        the cartridge — the game's own menu, waiting for the player
#   <slug>.png        the cover — a picture OF THE GAME, from a second and later run
#   index.json        title, file, model, source, both recipes and both readings, per game
#   _all.png          the contact sheet — every cover tiled, so the shelf can be seen at once
#
# ## Two runs per game, because the cartridge and the cover are different moments
#
# **They used to be one instant and that was wrong.** `--snapshot` was built so the picture and the
# machine state could not disagree — same frame, no frames between — and as a property that is
# still true of any single run. What it produced was a shelf of *menus*: `head-over-heels.png` was
# a grey screen reading SELECT JOYSTICK, `r-type.png` was a list of trainer pokes. Nobody
# recognises a game from a list of choices, and a cover nobody recognises is a cover that does not
# work.
#
# So each row is forged twice. **Run 1 stops at the menu and writes the `.z80`** — the cartridge
# still opens where the player gets to choose. **Run 2 goes further and writes the `.png`** — into
# the game, or as far as the game's own artwork. Same machine, same cassette, same load; only the
# keys after it and the settle differ, which is what the `cover-keys` and `cover-settle` columns
# carry.
#
# A cover is a frame of the game being played, or the game's own drawn artwork — the commissioned
# loading screen every Spectrum game paints while it loads, or a title screen that is a picture. A
# menu, a poke list, loading stripes, a blank screen or a publisher's logo is **not** a cover, and
# a row whose picture is one of those is not finished.
#
# ## Where the games come from, including the ones inside archives
#
# Fourteen of them arrived as `.zip`. This script extracts each one into its own scratch directory
# and reads the tape from there; **the owner's folder is opened and never written to**, and nothing
# is unpacked inside the repository. A `zip:` source names the archive and the member, because
# several archives carry a loading-screen `.SCR`, an `INFO/*.TXT` and a `POKES/*.POK` beside the
# tape, and picking by extension alone would have to guess between `Freddy Hardest - Side 1` and
# `Side 2`. Naming the member is one field and cannot guess wrong.
#
# **`cartridges/` is gitignored, and that is not a convenience.** A cartridge is a copy of somebody
# else's game, carrying its code and its screen; `.gitignore`'s own comment on `testdata/games/`
# says what is at stake — *"committing by default means redistributing somebody else's game"* — and
# this script is a machine for producing sixteen more of them. The rule is written down there,
# beside that one and with the same reasoning, rather than here where a reader of `.gitignore`
# would never find it.
#
# ## The recipes are in `tools/cartridges.txt`, and they were found by looking
#
# One line per game: which file, which machine, what to type, how long to wait. That file's header
# has the format and says why its `--settle` numbers are readings rather than constants. Nothing
# here could derive them: whether the instant after a tape runs out is a title screen, a menu or a
# black frame mid-clear is a property of the game, so each row was run, its cover looked at, its
# number moved, and run again. That loop is what `docs/images/README.md` publishes for the gallery
# and what this script exists to make cheap for a shelf.
#
# **The tape is asked rather than guessed at.** A row that types after its load passes
# `--keys-after`, which runs frames until the pulse train is spent and prints *"the tape ran out at
# frame N"*; that number is read back out of the log below and lands in the index as
# `tape_frames`. It is the honest per-game length — 9,208 frames for *Batty*, 31,728 for *Rex*
# side 1, 28,779 for *R-Type* — and it means no row has to carry a guess at how long a cassette is.
#
# ## What a cartridge is supposed to open on, which is NOT the game
#
# **The target is the game's own menu — the screen where the player picks the control scheme and
# the mode — and not the game already in play.** A cartridge that opens mid-level has taken the
# choice away: it has hard-coded whichever control scheme the recipe's key script happened to
# select, and it skips the screen a person actually wants when they plug a cartridge in. So every
# recipe here presses whatever is needed to *reach* that menu — some games need a key to leave a
# loading screen or an attract loop — and then stops before the key that would start play.
#
# The `verdict` column says which of three things the CARTRIDGE opens on:
#
#   menu     the game's own options screen, control schemes and modes visible. The target.
#   title    the game offers no control choice at all, and this is the screen where the player
#            has control — *Manic Miner*'s PRESS ENTER TO START with its key legend. Acceptable,
#            and the `menu` column says so in as many words.
#   blocked  none of the above was reachable. An honest gap.
#
# The `cover-shows` column is the other half and answers a different question: what is in the
# picture. It is written by somebody who opened the PNG, and if the honest answer to *"would a
# person who has never seen this game recognise it from this?"* is no, the row is not done.
#
# ## Two kinds of failure, and this script can only see one of them
#
# `zx-shot` exits 0 for a machine sitting on a crack's advertisement exactly as it does for a
# machine sitting on *Exolon*'s menu. Both ran, both rendered, both wrote a `.z80`; the difference
# is only in the picture. So the `verdict` and `menu` columns are written by **a person who opened
# the cover**, and this script reads them rather than deciding them. A run therefore reports three
# things and not two — the rows the tool could not forge at all (`FAIL`), the rows it forged into
# something that is not a menu (`GAP`), and the rest.
#
# A gap keeps its record, its cover and its tile. The temptation is to drop it, and a shelf with a
# silent hole in it is a shelf that lies about what this emulator can load — `docs/STATUS.md`'s
# standing complaint is about that exact shape. The tile is drawn with a red title instead of a
# white one, so the whole shelf can be judged in one look at `_all.png` without reading anything.
#
# ## Why this does not stop at the first failure
#
# `web/build.sh` is `set -eu` because a page assembled from half its parts is not a page. This is
# the other shape: fifteen cartridges and one honest gap is a useful afternoon, and stopping at the
# first game with an awkward loader would throw away the fourteen already forged. So it is
# `set -u`, every status is captured on the line that produced it, and a verdict at the end counts
# what happened — the dialect `web/gate.sh` uses, for the same reason.

set -u

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
root=$(CDPATH= cd -- "$here/.." && pwd)
manifest="$here/cartridges.txt"
out="$root/cartridges"
shot="$root/target/release/zx-shot"

# The `LOAD ""` script, spelled once. `J` is the whole keyword because a 48K starts a line in `K`
# mode, and each `LeftControl+P` is one `SYMBOL SHIFT`+`P`, which is a quote. It is the string
# every tape command in `docs/images/README.md` uses and the one `zx-shot`'s own `mod tests` grades
# its `--hold` edges with; the manifest writes `LOAD` in its keys column and this is what that
# expands to, so the four taps exist once in this repository's shell rather than sixteen times.
LOAD='J;LeftControl+P;LeftControl+P;Enter'

# Covers per row on the contact sheet. Four across at 320 pixels each is 1280 wide, which a laptop
# shows without scaling it — and a scaled contact sheet is one whose pixels are no longer the
# machine's.
ACROSS=4

usage() {
    cat >&2 <<'EOF'
usage: sh tools/forge.sh SOURCE-DIRECTORY

SOURCE-DIRECTORY holds the game files; the `source` column of tools/cartridges.txt names each
one relative to it. It is only ever read from.

There is no default, deliberately. The games this table names are the owner's own files and
live outside this repository, so a default would be one machine's path, committed, and wrong
on every other machine.
EOF
}

if [ "$#" -ne 1 ]; then usage; exit 2; fi
source_dir=$(CDPATH= cd -- "$1" 2>/dev/null && pwd) || {
    echo "forge: cannot read $1" >&2
    usage
    exit 2
}

cartridges=0
gaps=0
failures=0
step() { printf '\n==> %s\n' "$*"; }
ok()   { printf '    ok   %s\n' "$*"; }
gap()  { printf '    GAP  %s\n' "$*"; }
bad()  { printf '    FAIL %s\n' "$*"; }

# Strip the spaces the manifest's columns are lined up with. Leading and trailing only: a source
# name's internal spaces are its own, and `Head Over Heels .tap` is spelled with one before the
# extension — this must not be the thing that loses it.
trim() { printf '%s' "$1" | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//'; }

# A JSON string body: the two characters that can end one early, escaped. Not a general encoder —
# it does not reach control characters — and it does not have to be: every value it is handed came
# off a `|`-delimited line of a text file in this repository, and a control character in one would
# already have broken the read. Said out loud rather than left as an assumption somebody extends
# this past.
json() { printf '%s' "$1" | sed -e 's/\\/\\\\/g' -e 's/"/\\"/g'; }

# One `zx-shot` run, and the two answers a caller needs back from it.
#
# # Why a function and not the argument list written twice
#
# Because it *was* written twice the moment a row started needing two runs, and the two copies
# differ by exactly two flags — which is the shape this repository keeps finding: a second copy of
# a decision is a second thing to forget to change. The machine, the cassette, the `LOAD ""` and
# the PLAY are the same in both runs by construction now, so a cover cannot quietly be a picture
# of a different load from its own cartridge.
#
# `$1` names the run and so its files; `$2` is the after-keys, `$3` the settle, `$4` a `.z80` path
# or empty for none. It reports through `shoot_reason` and `shoot_tape` rather than by printing,
# because the caller decides whether a failure is this row's or the shelf's.
shoot() {
    shoot_reason=""
    shoot_tape=""
    set --
    for rom in $roms; do set -- "$@" --rom "$root/$rom"; done
    # The media and the output go on as separate arguments rather than as one string, so a name
    # with a space in it stays one argument. Half the names in this table have one.
    set -- "$@" --media "$file" --out "$work/$shoot_tag.ppm"
    [ -n "$shoot_snapshot" ] && set -- "$@" --snapshot "$shoot_snapshot"
    [ -n "$keys" ] && set -- "$@" --keys "$keys"
    [ -n "$play" ] && set -- "$@" "$play"
    [ -n "$shoot_after" ] && set -- "$@" --keys-after "$shoot_after"
    [ -n "$shoot_settle" ] && set -- "$@" --settle "$shoot_settle"
    [ -n "$hold" ] && set -- "$@" --hold "$hold"

    "$shot" "$@" > "$work/$shoot_tag.log" 2>&1 < /dev/null
    status=$?
    if [ "$status" -ne 0 ]; then
        # `zx-shot`'s refusal is its first line of stderr; the usage block after it is not the
        # news. Read out of the log rather than re-derived, so what is recorded is what the binary
        # actually said.
        shoot_reason=$(head -n 1 "$work/$shoot_tag.log")
        [ -n "$shoot_reason" ] || shoot_reason="zx-shot exited $status with nothing to say"
        return 1
    fi
    # The cassette's own length, in the run's own words. `--keys-after` prints it because it had to
    # compute it in order to know when to type; a run without that flag prints nothing and records
    # nothing, which is the truth about that run rather than a hole in this one.
    shoot_tape=$(sed -n 's/.*the tape ran out at frame \([0-9][0-9]*\).*/\1/p' "$work/$shoot_tag.log")
    return 0
}

# The flags that produced a file, as a line somebody can run. `--out` and `--snapshot` are left
# off: they are this script's own naming and say nothing about the game.
recipe_for() {
    recipe_out=""
    for rom in $roms; do recipe_out="$recipe_out --rom $rom"; done
    case "$source" in
        repo:*) recipe_out="$recipe_out --media '${source#repo:}'" ;;
        zip:*)  recipe_out="$recipe_out --media '${source#zip:}'" ;;
        *)      recipe_out="$recipe_out --media '$source'" ;;
    esac
    [ -n "$keys" ] && recipe_out="$recipe_out --keys '$keys'"
    [ -n "$play" ] && recipe_out="$recipe_out $play"
    [ -n "$1" ] && recipe_out="$recipe_out --keys-after '$1'"
    [ -n "$2" ] && recipe_out="$recipe_out --settle $2"
    [ -n "$hold" ] && recipe_out="$recipe_out --hold $hold"
    printf '%s' "${recipe_out# }"
}

# Where the intermediate frames and the per-game logs go. Removed on a clean run; kept when
# anything was blocked, because a tape load's output is long and only its last line is usually the
# news — so a failure prints the path of its log rather than the log.
work=$(mktemp -d "${TMPDIR:-/tmp}/zx-forge.XXXXXXXX") || exit 1
entries="$work/entries.json"
: > "$entries"

step "building zx-shot"
cargo build --release --manifest-path "$root/crates/frontend/Cargo.toml" --bin zx-shot
status=$?
if [ "$status" -ne 0 ]; then
    bad "cargo build (exit $status) — nothing can be forged without the binary"
    exit 1
fi
[ -x "$shot" ] || { bad "no zx-shot at $shot"; exit 1; }

mkdir -p "$out"

# ---------------------------------------------------------------------------------------
# One game per line.
# ---------------------------------------------------------------------------------------
# The manifest is redirected into the loop rather than piped into it, so the body runs in *this*
# shell and the counters it increments survive it. `zx-shot` gets `/dev/null` on stdin for a
# related reason: a child that inherited the manifest would eat the rest of the table.
tiles=""
separator=""
while IFS='|' read -r slug title source model keys settle hold after verdict menu \
                     coverkeys coversettle covershows note; do
    slug=$(trim "${slug:-}")
    case "$slug" in ''|'#'*) continue;; esac

    title=$(trim "${title:-}")
    source=$(trim "${source:-}")
    model=$(trim "${model:-}")
    keys=$(trim "${keys:-}")
    settle=$(trim "${settle:-}")
    hold=$(trim "${hold:-}")
    after=$(trim "${after:-}")
    verdict=$(trim "${verdict:-}")
    menu=$(trim "${menu:-}")
    coverkeys=$(trim "${coverkeys:-}")
    coversettle=$(trim "${coversettle:-}")
    covershows=$(trim "${covershows:-}")
    note=$(trim "${note:-}")

    step "$slug — $title"
    # `repo:` names a file in this repository instead of the owner's folder. One row needs it and
    # the reason is written down rather than left to be inferred: `batty` could not be forged, and
    # its cartridge is a snapshot the owner took by hand, which lives here rather than among his
    # game files. Everything else resolves against the directory this script was pointed at.
    reason=""
    tape=""
    case "$source" in
        repo:*)
            file="$root/${source#repo:}"
            ;;
        zip:*)
            # `zip:ARCHIVE!MEMBER`. Unpacked into this run's scratch directory, one directory per
            # archive so two archives holding the same inner name cannot collide, and never into
            # the owner's folder — his files are read and nothing else. `-o` is safe here because
            # the destination is ours and empty; `-q` because the interesting output is the game.
            archive=${source#zip:}
            member=${archive#*!}
            archive=${archive%%!*}
            unpacked="$work/unpacked/$slug"
            mkdir -p "$unpacked"
            unzip -o -q "$source_dir/$archive" "$member" -d "$unpacked" 2>> "$work/$slug.unzip.log"
            status=$?
            if [ "$status" -ne 0 ]; then
                reason="unzip exited $status on $archive!$member"
            fi
            file="$unpacked/$member"
            ;;
        *)
            file="$source_dir/$source"
            ;;
    esac

    # An unreadable verdict is refused rather than defaulted. Defaulting it either way is the one
    # mistake that matters: to `playing` and a gap becomes invisible, to `blocked` and a working
    # cartridge is disowned.
    case "$verdict" in
        menu|title|blocked) ;;
        *) reason="verdict '$verdict' is not one of menu, title, blocked" ;;
    esac
    if [ -z "$reason" ] && [ -z "$menu" ]; then
        # An empty `menu` is refused rather than written out as an empty string, because that
        # field is the evidence the cartridge reached its target: a picker reads it, and a row
        # nobody opened is exactly the row that would have nothing to put in it.
        reason="the menu column is empty, so nobody has said what this cartridge opens on"
    fi
    if [ -z "$reason" ] && [ "$verdict" != blocked ] && [ -z "$covershows" ]; then
        # And the same refusal for the other half. The owner's rule is that a cover is a picture
        # of the game; the only thing that can enforce it is a person having looked, and this
        # column is where that person writes down what they saw.
        reason="the cover-shows column is empty, so nobody has said what the picture shows"
    fi

    # `LOAD` in the keys column is the four taps of `LOAD ""`, expanded here so that script exists
    # in one place. Anything else passes through untouched: `Enter` for the 128's own boot menu,
    # `Key1` for a game already in memory.
    [ "$keys" = LOAD ] && keys=$LOAD

    # The ROMs, from the model. It is the *count* that names the machine — one is a 48K, two are a
    # 128 with the editor first — so this is the only place the two spellings appear, and there is
    # no `--model` flag anywhere for them to disagree with.
    case "$model" in
        48)  roms="testdata/roms/48.rom" ;;
        128) roms="testdata/roms/128-0.rom testdata/roms/128-1.rom" ;;
        *)   roms=""; [ -n "$reason" ] || reason="model '$model' is neither 48 nor 128" ;;
    esac

    # PLAY is derived and never a column, because it is not a choice: a cassette needs it and a
    # snapshot is already running. The extensions are the emulator's own; anything else is refused
    # here rather than by `zx-shot` three seconds later, so the message names the row that is wrong
    # instead of the file.
    play=""
    if [ -z "$reason" ]; then
        case "$file" in
            *.tap|*.TAP|*.tzx|*.TZX) play="--play-tape" ;;
            *.z80|*.Z80|*.sna|*.SNA) play="" ;;
            *) reason="$file is not a .tap, .tzx, .z80 or .sna" ;;
        esac
    fi

    if [ -z "$reason" ] && [ ! -f "$file" ]; then reason="no such file: $file"; fi

    # Both recipes, so each file in the shelf carries the command that made it.
    recipe=$(recipe_for "$after" "$settle")
    coverrecipe=$(recipe_for "$coverkeys" "${coversettle:-$settle}")

    # Run 1 — the cartridge. Stops at the menu; its frame is thrown away.
    if [ -z "$reason" ]; then
        shoot_tag="$slug-cartridge"; shoot_after="$after"; shoot_settle="$settle"
        shoot_snapshot="$out/$slug.z80"
        shoot || reason=$shoot_reason
        tape=$shoot_tape
    fi

    # Run 2 — the cover. The same load, carried further: `cover-keys` replaces the after-keys
    # outright rather than extending them, so a row whose best picture is its loading artwork can
    # say *"no keys at all, just stop here"* with an empty script and a mid-cassette settle.
    #
    # It writes no `.z80`. A snapshot of a machine mid-explosion is not a cartridge, and writing
    # one would mean the shelf held two files claiming to be the same thing.
    if [ -z "$reason" ]; then
        shoot_tag="$slug-cover"; shoot_after="$coverkeys"; shoot_settle="${coversettle:-$settle}"
        shoot_snapshot=""
        shoot || reason=$shoot_reason
    fi

    # **The cover must not be the cartridge's own frame.** A row whose second run changed nothing
    # produces two identical pictures, and the shelf then shows a menu where it promised a game —
    # which is precisely the failure this whole two-run shape exists to prevent, and precisely the
    # one nobody can see by reading the manifest. `cmp` can see it. A blocked row is exempt: it has
    # no second moment to reach, and its tile says NOT FORGED across the picture.
    if [ -z "$reason" ] && [ "$verdict" != blocked ]; then
        if cmp -s "$work/$slug-cartridge.ppm" "$work/$slug-cover.ppm"; then
            reason="the cover is byte-identical to the cartridge frame, so the second run changed \
nothing and this row would ship a picture of its own menu"
        fi
    fi

    if [ -z "$reason" ]; then
        # 1:1, and no `pamenlarge`. `docs/images/README.md` doubles its frames because a browser
        # would otherwise shrink them; a cover is handed to a picker that can scale it itself, and
        # at 320 x 256 this file **is** the frame rather than a rendering of one.
        pnmtopng "$work/$slug-cover.ppm" > "$out/$slug.png" 2> "$work/$slug.png.log"
        status=$?
        [ "$status" -eq 0 ] || reason="pnmtopng exited $status: $(cat "$work/$slug.png.log")"
    fi

    if [ -z "$reason" ]; then
        # The tile for the contact sheet is made here, where the title is already in hand, and
        # `ppmlabel` writes into a copy — so the sheet is legible and the committed cover is never
        # one with writing on it. Order is the manifest's, which groups a game with its siblings.
        #
        # A gap's title is drawn in red. That is the whole reporting surface of the contact sheet:
        # no legend, no second image, no reading required — the shelf is judged at a glance and a
        # red name is the invitation to go and read why.
        # A gap is titled in red AND says so across the middle of its own picture. Red alone was
        # not enough: a gap's frame can be the game's own logo — *R-Type*'s is — and a tile that
        # looks like every finished tile beside it reads as a finished row however its name is
        # coloured. The words are on the picture because that is what a person actually looks at.
        if [ "$verdict" = blocked ]; then
            ppmlabel -x 3 -y 12 -size 11 -color red -background black -text "$title" \
                -x 3 -y 130 -size 13 -color red -background black -text "NOT FORGED - see index" \
                "$work/$slug-cover.ppm" > "$work/tile-$slug.ppm" 2> "$work/$slug.label.log"
        else
            ppmlabel -x 3 -y 12 -size 11 -color white -background black -text "$title" \
                "$work/$slug-cover.ppm" > "$work/tile-$slug.ppm" 2> "$work/$slug.label.log"
        fi
        status=$?
        if [ "$status" -eq 0 ]; then tiles="$tiles $work/tile-$slug.ppm"
        else bad "ppmlabel on $slug (exit $status): $(cat "$work/$slug.label.log")"; fi
    fi

    # Three outcomes, not two. `state` is what the index records, and it is the manifest's verdict
    # unless the tool never got far enough for a verdict to mean anything.
    if [ -n "$reason" ]; then
        state=failed
        bad "$reason"
        printf '         log: %s\n' "$work/$slug.log"
        failures=$((failures + 1))
        # A cartridge left by an earlier run must not survive a row that has since stopped working:
        # the index would say failed, the directory would hold a file, and the file would be
        # believed.
        rm -f "$out/$slug.z80" "$out/$slug.png"
    elif [ "$verdict" = blocked ]; then
        state=blocked
        gap "$slug.png — $note"
        gaps=$((gaps + 1))
        # The cover stays and the `.z80` goes. A snapshot of a machine parked on a crack's
        # advertisement is evidence, and evidence belongs in the log and the picture — but a
        # `.z80` sitting in `cartridges/` next to fifteen real ones will be double-clicked by
        # somebody who has not read the index, and it is not a cartridge.
        rm -f "$out/$slug.z80"
    else
        state=$verdict
        ok "$slug.z80 opens on the $verdict — $menu"
        printf '         cover: %s\n' "$covershows"
        cartridges=$((cartridges + 1))
    fi

    # The separator goes in front of a record rather than behind it, which is what lets the last
    # one be written without knowing that it is the last.
    printf '%s' "$separator" >> "$entries"
    separator=',
'
    {
        printf '    {\n'
        printf '      "slug": "%s",\n' "$(json "$slug")"
        printf '      "title": "%s",\n' "$(json "$title")"
        printf '      "state": "%s",\n' "$state"
        case "$state" in
            menu|title) printf '      "cartridge": "%s.z80",\n' "$(json "$slug")" ;;
            *)          printf '      "cartridge": null,\n' ;;
        esac
        case "$state" in
            failed) printf '      "cover": null,\n' ;;
            *)      printf '      "cover": "%s.png",\n' "$(json "$slug")" ;;
        esac
        printf '      "model": "%s",\n' "$(json "$model")"
        printf '      "source": "%s",\n' "$(json "$source")"
        printf '      "tape_frames": %s,\n' "${tape:-null}"
        printf '      "recipe": "%s",\n' "$(json "$recipe")"
        printf '      "menu": "%s",\n' "$(json "$menu")"
        printf '      "cover_recipe": "%s",\n' "$(json "$coverrecipe")"
        printf '      "cover_shows": "%s",\n' "$(json "$covershows")"
        if [ "$state" = failed ]; then
            printf '      "forge_error": "%s",\n' "$(json "$reason")"
        else
            printf '      "forge_error": null,\n'
        fi
        printf '      "note": "%s"\n' "$(json "$note")"
        printf '    }'
    } >> "$entries"
done < "$manifest"
printf '\n' >> "$entries"

# ---------------------------------------------------------------------------------------
# The index.
# ---------------------------------------------------------------------------------------
# Every field a picker needs to list the shelf, and every field somebody needs to rebuild one
# cartridge from its own record without opening this script or the manifest.
step "cartridges/index.json"
{
    printf '{\n'
    printf '  "forged_by": "tools/forge.sh, from tools/cartridges.txt",\n'
    printf '  "source_directory": "%s",\n' "$(json "$source_dir")"
    printf '  "cartridges": [\n'
    cat "$entries"
    printf '  ]\n'
    printf '}\n'
} > "$out/index.json"
ok "$cartridges cartridges, $gaps gaps, $failures failed to run"

# ---------------------------------------------------------------------------------------
# The contact sheet.
# ---------------------------------------------------------------------------------------
# Every cover at 320 x 256, four across, each with its title on it.
#
# Not `pnmindex`, which would do the whole thing in one command: it scales through `pamscale`, and
# a resampled Spectrum pixel is not a Spectrum pixel. That is the hazard `docs/images/README.md`
# names as the one its entire page exists to exclude, and it applies to a working artefact as much
# as to a published one — the sheet is what a person forms an opinion from.
step "cartridges/_all.png"
if [ -z "$tiles" ]; then
    bad "no covers to tile — the shelf is empty"
else
    rows=""
    row=""
    n=0
    for tile in $tiles; do
        row="$row $tile"
        n=$((n + 1))
        if [ "$((n % ACROSS))" -eq 0 ]; then
            # shellcheck disable=SC2086  # deliberate word splitting: one tile per argument
            pnmcat -lr -jtop -black $row > "$work/row-$n.ppm"
            rows="$rows $work/row-$n.ppm"
            row=""
        fi
    done
    if [ -n "$row" ]; then
        # The last row, short. `-jleft -black` on the stack below pads it out to the full width, so
        # a shelf whose count is not a multiple of four still tiles.
        # shellcheck disable=SC2086
        pnmcat -lr -jtop -black $row > "$work/row-last.ppm"
        rows="$rows $work/row-last.ppm"
    fi
    # shellcheck disable=SC2086
    pnmcat -tb -jleft -black $rows > "$work/all.ppm"
    status=$?
    if [ "$status" -ne 0 ]; then
        bad "pnmcat could not stack the rows (exit $status)"
    else
        pnmtopng "$work/all.ppm" > "$out/_all.png"
        status=$?
        if [ "$status" -eq 0 ]; then
            ok "_all.png — $((cartridges + gaps)) covers, $ACROSS across, gaps titled in red"
        else bad "pnmtopng on the contact sheet (exit $status)"; fi
    fi
fi

# ---------------------------------------------------------------------------------------
step "the shelf"
printf '    %s\n' "$out"
ls -l "$out"

if [ "$gaps" -eq 0 ] && [ "$failures" -eq 0 ]; then
    printf '\n    %s cartridges, and every one opens on a screen the player can choose from.\n\n' \
        "$cartridges"
    rm -rf "$work"
    exit 0
fi
printf '\n    %s cartridges, %s gaps, %s that would not run.\n' "$cartridges" "$gaps" "$failures"
printf '    Every one of them is in index.json with what was reached and what stopped it, and\n'
printf '    every gap kept its cover so the reason can be looked at rather than read about.\n'
printf '    This exits non-zero because an incomplete shelf is a fact about the shelf rather\n'
printf '    than about the run. Logs, kept for that: %s\n\n' "$work"
exit 1
