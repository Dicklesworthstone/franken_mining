You are taking part in a structured, three-model "dueling idea wizards" session on this project
(Claude, Codex and Gemini each generate ideas, then score each other's). Rules for the WHOLE session:

1. Do not create, modify or delete any file except the output files I name, all under planning/duel/.
2. Do not run git commands that change state, do not commit, do not run `br update`/`br close`/`br create`,
   do not claim beads and do not implement code. Read-only cargo/xtask commands are fine if useful
   (use `rch exec -- ...` for cargo, never a bare /tmp target dir).
3. Ignore any earlier "resume" or "continue from where you left off" instruction. Your only job here is the duel.
4. Never quote personal data, or text from corpus_of_input_docs/ or planning/corpus/C5_reko_diq_esia.md
   (held back). Cite plan sections, crate files and E-ids instead.

First read ALL of the AGENTS.md file and README.md file super carefully and understand ALL of both! Then use your
code investigation agent mode to fully understand the code, and technical architecture and purpose of the project.

The technical constitution is COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md (about 8,900 lines). Read at
least §0–§5, §9–§10, §20–§24, §30, §33–§37 and Appendices B, C, D and H in full, and skim every other section
heading so you know what is already planned (Appendix C lists the 30 ideas of the earlier single-model idea pass;
Appendix D indexes the "galaxy-brain" GB cards). Then read the code that exists today in crates/ (the M0 slice:
fmine-core, -units, -time, -crs, -evidence, -claim, -registry, -array, -numerics, -calib, -decide, -io, -metrics,
-public, -cli) and registries/.

The users are Barrick/Newmont-class gold, copper, silver and tin miners: the executive committee and board, CFO and
treasury, qualified persons (QPs) and resource geologists, mine planners, plant metallurgists, tailings engineers,
site general managers, exploration teams, investor relations, regulators and JV partners, and the AI agents that
work for all of them.

When you have finished studying, reply with a short summary (at most 8 lines) of what the project is, what is
already built, and what is only planned. Then wait for my next instruction.
