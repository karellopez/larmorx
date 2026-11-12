# The documentation revision: the old words

`old_words.py` counts, file by file, in this repository and in a checkout of the documentation
site (`larmorx_documentation`), the words that [docs/writing-guide.md](../../../docs/writing-guide.md)
replaces: *clean-room original*, *original* where it names a clean room, *exact replica*, *port*
as a kind of its own, the slogans, and the developers' jargon on the pages users read. Every
other *original* is counted as a word to read. It also lists the files whose name holds
*original* and how many files name each. The result is [inventory.md](inventory.md), from which
the revision's rollout is planned (docs/documentation-plan.md, step 4).

```
python -P validation/investigations/docs-revision/old_words.py --repo . \
    --site <checkout of larmorx_documentation> \
    --out validation/investigations/docs-revision/inventory.md
```

It reads tracked files only and takes about 15 seconds. Run again after each step of the rollout:
the columns of words to change should fall to zero in the areas the writer changes, and stay in
the areas kept as written (history, identifiers).
