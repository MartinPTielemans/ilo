#ifndef BROWSER_CORE_AUTO_ARCHIVE_H_
#define BROWSER_CORE_AUTO_ARCHIVE_H_

#include "base/time/time.h"

class Profile;

namespace browser_core {

// Arc's auto-archive: a Today tab no window has shown for 12 hours, or the time the user chose for
// its Space's profile in Settings › Profiles, closes into the archive, but for one playing sound. One timer waits for the tab unseen
// longest, on the wall clock, so it keeps time through sleep and a quit (the Spaces keep when each
// tab was last seen).
//
// Little ilo windows go the same way, as Arc's Little Arc windows do: one that hasn't been the
// active window for 6 hours, or the time chosen in Settings › General (bc_little_archive_after),
// closes into the archive with its tab, but for one whose page plays sound. The same timer waits
// for the one due first.
//
// Looks again for that tab, from a task of its own, as the Spaces, the tabs windows show, a Little
// ilo window that stops being active or the time chosen change.
void ScheduleAutoArchive();

// Registers the pref that keeps each profile's time. Before the first profile is made.
void RegisterArchivePrefs();
// Gives |profile|, if it has no time of its own, the default the user chose in Settings › General,
// as Arc's new profiles start with it. Not until the Spaces, which keep the default, are read.
void GiveArchiveDefault(Profile* profile);
// The Spaces were read: gives the default to the profiles loaded before, and those loaded after.
void ArchiveDefaultRead();
// |profile|'s time, or the default for one that has none yet.
base::TimeDelta ProfileArchiveAfter(Profile* profile);
// Sets |profile|'s time, and returns whether it did: not for no profile or a time that isn't
// positive.
bool SetProfileArchiveAfter(Profile* profile, base::TimeDelta after);
// The note on the first tabs auto-archive takes: shown from then until the user dismisses it, and
// never again after that (bc_archive_note).
bool ArchiveNoteShown();
void DismissArchiveNote();

}  // namespace browser_core

#endif  // BROWSER_CORE_AUTO_ARCHIVE_H_
