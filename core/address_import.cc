#include "browser_core/address_import.h"

#include <algorithm>
#include <optional>
#include <string>
#include <utility>
#include <vector>

#include "base/functional/bind.h"
#include "base/location.h"
#include "base/memory/raw_ref.h"
#include "base/memory/weak_ptr.h"
#include "base/scoped_observation.h"
#include "base/strings/utf_string_conversions.h"
#include "base/task/sequenced_task_runner.h"
#include "browser_core/host.h"
#include "chrome/browser/autofill/personal_data_manager_factory.h"
#include "chrome/browser/browser_process.h"
#include "chrome/browser/profiles/profile.h"
#include "chrome/browser/profiles/profile_observer.h"
#include "components/autofill/core/browser/country_type.h"
#include "components/autofill/core/browser/data_manager/addresses/address_data_manager.h"
#include "components/autofill/core/browser/data_manager/personal_data_manager.h"
#include "components/autofill/core/browser/data_model/addresses/autofill_profile.h"
#include "components/autofill/core/browser/data_model/addresses/autofill_structured_address_component.h"
#include "components/autofill/core/browser/field_types.h"

namespace browser_core {

namespace {

using autofill::AddressDataManager;
using autofill::AutofillProfile;

// An address as Chromium's Web Data table makes one from its type tokens
// (AddressAutofillTable::GetAutofillProfile): its country first, then each field it keeps.
AutofillProfile ProfileOf(const bc_address& address) {
  std::string country;
  for (size_t i = 0; i < address.count; ++i) {
    if (address.fields[i].type == autofill::ADDRESS_HOME_COUNTRY && address.fields[i].value) {
      country = address.fields[i].value;
    }
  }
  AutofillProfile profile(AutofillProfile::RecordType::kLocalOrSyncable,
                          autofill::AddressCountryCode(country));
  for (size_t i = 0; i < address.count; ++i) {
    const bc_address_field& field = address.fields[i];
    std::optional<autofill::FieldType> type = autofill::ToSafeFieldType(field.type);
    std::optional<autofill::VerificationStatus> status =
        autofill::ToSafeVerificationStatus(field.status);
    if (!type || !status || !field.value ||
        !AutofillProfile::kDatabaseStoredTypes.contains(*type)) {
      continue;
    }
    profile.SetRawInfoWithVerificationStatus(*type, base::UTF8ToUTF16(field.value), *status);
  }
  profile.FinalizeAfterImport();
  return profile;
}

// Waits for the profile's addresses to load, as the store reads them after the profile opens, so
// it knows which it has. It deletes itself once it's added them, or if the profile goes first:
// the store tells its observers nothing as it shuts down.
class AddAddresses : public AddressDataManager::Observer, public ProfileObserver {
 public:
  AddAddresses(Profile* profile,
               AddressDataManager& addresses,
               bc_id id,
               std::vector<AutofillProfile> add)
      : addresses_(addresses), id_(id), add_(std::move(add)) {
    profile_observation_.Observe(profile);
    addresses_observation_.Observe(&addresses);
    // Never before bc_profile_add_addresses returns, so the app has its answer first.
    base::SequencedTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE, base::BindOnce(&AddAddresses::OnAddressDataChanged,
                                  weak_factory_.GetWeakPtr()));
  }

  // AddressDataManager::Observer:
  void OnAddressDataChanged() override {
    if (!addresses_->has_initial_load_finished()) {
      return;
    }
    const std::string& locale = g_browser_process->GetApplicationLocale();
    // AddProfile adds in a task of its own, so GetProfiles doesn't yet list those added here.
    std::vector<const AutofillProfile*> had = addresses_->GetProfiles();
    std::vector<const AutofillProfile*> added;
    for (const AutofillProfile& profile : add_) {
      auto same = [&profile](const AutofillProfile* each) { return each->Compare(profile) == 0; };
      if (profile.IsEmpty(locale) || std::ranges::any_of(had, same) ||
          std::ranges::any_of(added, same)) {
        continue;
      }
      addresses_->AddProfile(profile);
      added.push_back(&profile);
    }
    Done(added.size());
  }

  // ProfileObserver:
  void OnProfileWillBeDestroyed(Profile* profile) override { Done(0); }

 private:
  void Done(size_t added) {
    if (Host().addresses_added) {
      Host().addresses_added(Host().context, id_, added);
    }
    delete this;
  }

  const raw_ref<AddressDataManager> addresses_;
  const bc_id id_;
  const std::vector<AutofillProfile> add_;
  base::ScopedObservation<Profile, ProfileObserver> profile_observation_{this};
  base::ScopedObservation<AddressDataManager, AddressDataManager::Observer>
      addresses_observation_{this};
  base::WeakPtrFactory<AddAddresses> weak_factory_{this};
};

}  // namespace

bool AddImportedAddresses(Profile* profile, bc_id id, const bc_address* addresses, size_t count) {
  if (!profile || profile->IsOffTheRecord()) {
    return false;
  }
  autofill::PersonalDataManager* data =
      autofill::PersonalDataManagerFactory::GetForBrowserContext(profile);
  if (!data || !data->address_data_manager().IsAutofillProfileEnabled()) {
    return false;
  }
  std::vector<AutofillProfile> add;
  add.reserve(count);
  for (size_t i = 0; i < count; ++i) {
    add.push_back(ProfileOf(addresses[i]));
  }
  // Deletes itself.
  new AddAddresses(profile, data->address_data_manager(), id, std::move(add));
  return true;
}

}  // namespace browser_core
