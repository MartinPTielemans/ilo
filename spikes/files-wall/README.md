# Files wall (#4101)

**Question:** can an OS sandbox around the assistant's whole process hold the task's level on disk by itself, without relying on the assistant's own checks?

The levels' table, for a folder the user granted and for attached files:

| | Attached files | Folder |
|---|---|---|
| Look | read, search | read, search |
| Work | read, search | + make, change |
| Full | read, search | + delete, move, replace |

## How it tests

`wall.py` builds a throwaway tree for each level and derives the sandbox's rules from that one table. The tree holds:
- the folder;
- the attached copies;
- a place outside;
- a home of stand-ins for the assistant's own folder and for folders that are never granted.

It then starts `probe.py` inside the rules. `probe.py` is a stand-in process that tries every operation in every place and reports what the system answered.

The routes it tries:
- full paths and `..`;
- links to files and folders outside;
- handles opened for a path only;
- the process's root under `/proc`;
- hard links and moves in both directions;
- moving the granted folder itself.

`wall.py` also checks:
- the folder picker's rule for folders that are never granted, at the pick and again at the next start;
- that where the sandbox can't be applied, nothing starts.

Linux uses Landlock, the Mac Seatbelt. There's no real assistant and no sign-in: the stand-in is the whole test.

## Run it

```
python3 -I spikes/files-wall/wall.py [WORK]
```

`WORK` defaults to a new folder in the system's temp, and holds only what the spike makes.

It prints a table per level, the picker's cases and the fail-closed cases. It exits 1 if anything the table refuses got through.

## Where it stands

- **Linux (Landlock, kernel 6.8):** nothing the table refuses got through, at any level.
- **The Mac:** to run.
- **The real assistants inside the wall:** they wait for throwaway sign-ins.

The findings go on #4101.
