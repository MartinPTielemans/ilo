#include "browser_core/open_app_dialog.h"

#include <utility>

#include "base/functional/bind.h"
#include "base/strings/utf_string_conversions.h"
#include "base/types/optional_util.h"
#include "browser_core/agent_space_page.h"
#include "browser_core/tab.h"
#include "chrome/browser/external_protocol/external_protocol_handler.h"
#include "chrome/browser/profiles/profile.h"
#include "chrome/common/pref_names.h"
#include "chrome/grit/generated_resources.h"
#include "components/prefs/pref_service.h"
#include "components/url_formatter/elide_url.h"
#include "content/public/browser/navigation_handle.h"
#include "content/public/browser/web_contents.h"
#include "net/base/registry_controlled_domains/registry_controlled_domain.h"
#include "ui/base/l10n/l10n_util.h"
#include "ui/gfx/text_elider.h"

namespace browser_core {

// static
void OpenAppDialog::Ask(const GURL& url,
                        content::WebContents* web_contents,
                        const std::optional<url::Origin>& initiating_origin,
                        content::WeakDocumentPtr initiator_document,
                        const std::u16string& program_name) {
  // As Chrome's, which needs a tab's dialog host: with none, the page opens nothing.
  if (!Tab::FromWebContents(web_contents)) {
    return;
  }
  auto* dialog =
      new OpenAppDialog(web_contents, url, initiating_origin, std::move(initiator_document));
  PageDialog::Words words;
  words.kind = BC_DIALOG_OPEN_APP;
  // Chrome's elides the app's name to this in its title.
  constexpr size_t kMaxCommandCharsToDisplay = 32;
  std::u16string elided;
  gfx::ElideString(program_name, kMaxCommandCharsToDisplay, &elided);
  words.title = l10n_util::GetStringFUTF8(IDS_EXTERNAL_PROTOCOL_TITLE, elided);
  words.message =
      !initiating_origin || initiating_origin->opaque()
          ? l10n_util::GetStringUTF8(IDS_EXTERNAL_PROTOCOL_MESSAGE)
          : l10n_util::GetStringFUTF8(
                IDS_EXTERNAL_PROTOCOL_MESSAGE_WITH_INITIATING_ORIGIN,
                url_formatter::FormatOriginForSecurityDisplay(*initiating_origin));
  words.accept = l10n_util::GetStringFUTF8(IDS_EXTERNAL_PROTOCOL_OK_BUTTON_TEXT, program_name);
  words.cancel = l10n_util::GetStringUTF8(IDS_EXTERNAL_PROTOCOL_CANCEL_BUTTON_TEXT);
  // As Chrome's: the user may skip the question for the scheme from a trustworthy origin, unless
  // policy says not.
  Profile* profile = Profile::FromBrowserContext(web_contents->GetBrowserContext());
  if (profile->GetPrefs()->GetBoolean(prefs::kExternalProtocolDialogShowAlwaysOpenCheckbox) &&
      ExternalProtocolHandler::MayRememberAllowDecisionsForThisOrigin(
          base::OptionalToPtr(initiating_origin))) {
    words.check = l10n_util::GetStringFUTF8(
        IDS_EXTERNAL_PROTOCOL_CHECKBOX_PER_ORIGIN_TEXT,
        url_formatter::FormatOriginForSecurityDisplay(
            *initiating_origin, url_formatter::SchemeDisplay::OMIT_CRYPTOGRAPHIC));
  }
  dialog->id_ =
      PageDialog::Ask(Tab::IdOf(web_contents), std::move(words),
                      base::BindOnce(&OpenAppDialog::Answered, dialog->weak_factory_.GetWeakPtr()));
}

// static
bool OpenAppDialog::MayOpen(content::WebContents* contents) {
  if (!Tab::FromWebContents(contents)) {
    return true;
  }
  return !AgentSpaceOf(Tab::IdOf(contents));
}

OpenAppDialog::OpenAppDialog(content::WebContents* web_contents,
                             const GURL& url,
                             const std::optional<url::Origin>& initiating_origin,
                             content::WeakDocumentPtr initiator_document)
    : content::WebContentsObserver(web_contents),
      url_(url),
      initiating_origin_(initiating_origin),
      initiator_document_(std::move(initiator_document)) {}

OpenAppDialog::~OpenAppDialog() {
  PageDialog::Withdraw(id_);
}

void OpenAppDialog::Answered(const PageDialog::Reply& reply) {
  // Answered, so there's nothing to take back.
  id_ = 0;
  if (reply.accept && web_contents()) {
    if (reply.checked && initiating_origin_) {
      ExternalProtocolHandler::SetBlockState(
          url_.GetScheme(), *initiating_origin_, ExternalProtocolHandler::DONT_BLOCK,
          Profile::FromBrowserContext(web_contents()->GetBrowserContext()));
    }
    ExternalProtocolHandler::LaunchUrlWithoutSecurityCheck(url_, web_contents(),
                                                           initiator_document_);
  }
  delete this;
}

void OpenAppDialog::DidFinishNavigation(content::NavigationHandle* navigation) {
  // As Chrome's dialogs over a page, it goes once the page goes to another site.
  if (!navigation->IsInPrimaryMainFrame() || !navigation->HasCommitted() ||
      net::registry_controlled_domains::SameDomainOrHost(
          navigation->GetPreviousPrimaryMainFrameURL(), navigation->GetURL(),
          net::registry_controlled_domains::INCLUDE_PRIVATE_REGISTRIES)) {
    return;
  }
  delete this;
}

void OpenAppDialog::WebContentsDestroyed() {
  delete this;
}

}  // namespace browser_core
