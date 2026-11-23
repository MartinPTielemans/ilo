#include "browser_core/agent_sign_in_rule.h"

#include "testing/gtest/include/gtest/gtest.h"
#include "url/gurl.h"
#include "url/origin.h"

namespace browser_core {
namespace {

url::Origin Page() {
  return url::Origin::Create(GURL("https://shop.example.org/account/login?next=%2F"));
}

// A login the sign-in fills on Page(): the page's own, exact, with a password.
SavedLogin Good(const char16_t* username, bool account = false) {
  SavedLogin login;
  login.exact = true;
  login.html = true;
  login.has_password = true;
  login.account = account;
  login.realm = "https://shop.example.org/";
  login.username = username;
  return login;
}

TEST(AgentSignInRule, ThePagesOwnLoginIsUsable) {
  EXPECT_EQ(UsableLogins({Good(u"ana")}, Page()), (std::vector<size_t>{0}));
}

TEST(AgentSignInRule, OnlyAnHttpsPageGetsALogin) {
  EXPECT_TRUE(
      UsableLogins({Good(u"ana")}, url::Origin::Create(GURL("http://shop.example.org/"))).empty());
  EXPECT_TRUE(UsableLogins({Good(u"ana")}, url::Origin()).empty());
}

TEST(AgentSignInRule, AnotherSubdomainsLoginIsNot) {
  SavedLogin other = Good(u"ana");
  other.realm = "https://example.org/";
  EXPECT_TRUE(UsableLogins({other}, Page()).empty());
  other.realm = "https://shop.example.org:8443/";
  EXPECT_TRUE(UsableLogins({other}, Page()).empty());
}

TEST(AgentSignInRule, AnythingButAnExactMatchIsNot) {
  // The store's public-suffix, affiliated and grouped matches come with exact unset.
  SavedLogin near = Good(u"ana");
  near.exact = false;
  EXPECT_TRUE(UsableLogins({near}, Page()).empty());
}

TEST(AgentSignInRule, BlockedFederatedHttpAuthAndPasswordlessLoginsAreNot) {
  SavedLogin blocked = Good(u"ana");
  blocked.blocked = true;
  SavedLogin federated = Good(u"ben");
  federated.federated = true;
  SavedLogin http_auth = Good(u"cy");
  http_auth.html = false;
  SavedLogin empty = Good(u"di");
  empty.has_password = false;
  EXPECT_TRUE(UsableLogins({blocked, federated, http_auth, empty}, Page()).empty());
}

TEST(AgentSignInRule, TheAccountsComeFirstAndAUsernameCountsOnce) {
  std::vector<SavedLogin> logins = {Good(u"ana"), Good(u"ben", true), Good(u"ana", true)};
  // ana is in both stores: the account's is the one, so the user picks from two, not three.
  EXPECT_EQ(UsableLogins(logins, Page()), (std::vector<size_t>{1, 2}));
}

FormField Field(FormField::Kind kind, const char* autocomplete, bool visible = true) {
  FormField field;
  field.kind = kind;
  field.autocomplete = autocomplete;
  field.visible = visible;
  return field;
}

TEST(AgentSignInRule, AUsernameAloneIsTheFirstPage) {
  EXPECT_EQ(UsernameOnlyField({Field(FormField::Kind::kText, "search", false),
                               Field(FormField::Kind::kEmail, "section-a Username webauthn")}),
            1u);
  EXPECT_EQ(UsernameOnlyField({Field(FormField::Kind::kText, "email")}), 0u);
}

TEST(AgentSignInRule, AFormWithAPasswordFieldIsNot) {
  // Hidden too: a page that shows its password field after the username isn't username-first.
  EXPECT_FALSE(UsernameOnlyField({Field(FormField::Kind::kText, "username"),
                                  Field(FormField::Kind::kPassword, "", false)}));
}

TEST(AgentSignInRule, AFieldNotNamedAUsernameHiddenOrFixedIsNot) {
  EXPECT_FALSE(UsernameOnlyField({Field(FormField::Kind::kText, "")}));
  EXPECT_FALSE(UsernameOnlyField({Field(FormField::Kind::kText, "username", false)}));
  FormField fixed = Field(FormField::Kind::kText, "username");
  fixed.readonly = true;
  EXPECT_FALSE(UsernameOnlyField({fixed}));
  EXPECT_FALSE(UsernameOnlyField({Field(FormField::Kind::kOther, "username")}));
}

TEST(AgentSignInRule, TwoUsernameFieldsAreNoSuchForm) {
  EXPECT_FALSE(UsernameOnlyField(
      {Field(FormField::Kind::kText, "username"), Field(FormField::Kind::kEmail, "email")}));
}

TEST(AgentSignInRule, ALongPasswordShowsAnywhere) {
  EXPECT_TRUE(ShowsPassword(u"Echo: hunter22!", u"hunter22!"));
  EXPECT_TRUE(ShowsPassword(u"hunter", u"hunter"));
  EXPECT_FALSE(ShowsPassword(u"Echo: hunter2", u"hunter22!"));
}

TEST(AgentSignInRule, AShortPasswordShowsOnlyAsAllOfAText) {
  EXPECT_TRUE(ShowsPassword(u"  abc1 ", u"abc1"));
  EXPECT_FALSE(ShowsPassword(u"label abc1", u"abc1"));
}

TEST(AgentSignInRule, NoPasswordShowsNowhere) {
  EXPECT_FALSE(ShowsPassword(u"", u""));
  EXPECT_FALSE(ShowsPassword(u"anything", u""));
}

TEST(AgentSignInRule, ThePasswordFieldIsFoundByItsIdThenItsName) {
  EXPECT_EQ(FieldSelector(true, "pw", "password"), "input[type=password][id=\"pw\"]");
  EXPECT_EQ(FieldSelector(true, "", "password"), "input[type=password][name=\"password\"]");
  EXPECT_EQ(FieldSelector(true, "", ""), "input[type=password]");
}

TEST(AgentSignInRule, AUsernameFieldNeedsAnIdOrAName) {
  EXPECT_EQ(FieldSelector(false, "", "email"), "input[name=\"email\"]");
  EXPECT_EQ(FieldSelector(false, "", ""), std::nullopt);
}

TEST(AgentSignInRule, AFieldsIdCantEndItsSelector) {
  EXPECT_EQ(FieldSelector(true, "a\"]x", ""), "input[type=password][id=\"a\\\"]x\"]");
  EXPECT_EQ(FieldSelector(true, "a\nb", ""), "input[type=password][id=\"a\\a b\"]");
}

TEST(AgentSignInRule, AFormPostsWithinThePagesSite) {
  EXPECT_TRUE(PostsWithin(GURL(), Page()));
  EXPECT_TRUE(PostsWithin(GURL("https://shop.example.org/session"), Page()));
  EXPECT_TRUE(PostsWithin(GURL("https://login.example.org/session"), Page()));
}

TEST(AgentSignInRule, AFormPostingElsewhereIsNot) {
  EXPECT_FALSE(PostsWithin(GURL("https://collector.example.com/session"), Page()));
  EXPECT_FALSE(PostsWithin(GURL("http://shop.example.org/session"), Page()));
  EXPECT_FALSE(PostsWithin(GURL("javascript:void(0)"), Page()));
  EXPECT_FALSE(PostsWithin(GURL("not a url"), Page()));
  // Each name under a private registry, such as github.io, is a site of its own.
  EXPECT_FALSE(PostsWithin(GURL("https://other.github.io/session"),
                           url::Origin::Create(GURL("https://mine.github.io/login"))));
}

TEST(AgentSignInRule, AFailedSignInIsTheUsersStepThereForThatAgent) {
  const url::Origin other = url::Origin::Create(GURL("https://mail.example.org/"));
  SignInTries tries;
  EXPECT_TRUE(tries.MayTry("claude", Page()));
  tries.Failed("claude", Page());
  EXPECT_FALSE(tries.MayTry("claude", Page()));
  EXPECT_TRUE(tries.MayTry("claude", other));
  EXPECT_TRUE(tries.MayTry("codex", Page()));
}

TEST(AgentSignInRule, HandingTheOriginBackForgivesEveryAgentThere) {
  const url::Origin other = url::Origin::Create(GURL("https://mail.example.org/"));
  SignInTries tries;
  tries.Failed("claude", Page());
  tries.Failed("codex", Page());
  tries.Failed("claude", other);
  tries.Forgive(Page());
  EXPECT_TRUE(tries.MayTry("claude", Page()));
  EXPECT_TRUE(tries.MayTry("codex", Page()));
  EXPECT_FALSE(tries.MayTry("claude", other));
}

TEST(AgentSignInRule, AnotherOriginsHandBackForgivesNothing) {
  SignInTries tries;
  tries.Failed("claude", Page());
  // The same site's other host, and the page's host over http, are other origins.
  tries.Forgive(url::Origin::Create(GURL("https://login.example.org/")));
  tries.Forgive(url::Origin::Create(GURL("http://shop.example.org/")));
  EXPECT_FALSE(tries.MayTry("claude", Page()));
}

TEST(AgentSignInRule, AWithheldPageIsWithheldWhereverItComesBackFrom) {
  const GURL login("https://shop.example.org/login");
  const GURL echo("https://shop.example.org/session?u=ada&p=hunter2-by-ada");
  const GURL other("https://shop.example.org/help");
  WithheldHistory history;
  // The look held the login page, with the submit's navigation, 7, still under way.
  history.Withhold(login, {7});
  EXPECT_TRUE(history.shown());
  EXPECT_TRUE(history.Committed(7, echo, /*same_document=*/false));
  // Away: the mark lifts.
  EXPECT_FALSE(history.Committed(8, other, false));
  // Back, a reload, a redirect or a navigation to either address: withheld again, fragment or not.
  EXPECT_TRUE(history.Committed(9, echo, false));
  EXPECT_FALSE(history.Committed(10, other, false));
  EXPECT_TRUE(history.Committed(11, GURL(login.spec() + "#top"), false));
  // Another query is another address.
  EXPECT_FALSE(history.Committed(12, GURL("https://shop.example.org/login?next=%2F"), false));
}

TEST(AgentSignInRule, AWithheldDocumentStaysWithheldWhateverItsAddressBecomes) {
  const GURL login("https://shop.example.org/login");
  WithheldHistory history;
  history.Withhold(login, {});
  // The page moves its own address: the document is the same, and its new address is withheld.
  EXPECT_TRUE(history.Committed(1, GURL("https://shop.example.org/welcome"), true));
  EXPECT_FALSE(history.Committed(2, GURL("https://shop.example.org/help"), false));
  EXPECT_TRUE(history.Committed(3, GURL("https://shop.example.org/welcome"), false));
  // A page not withheld moving its address to a withheld one is withheld too.
  EXPECT_FALSE(history.Committed(4, GURL("https://shop.example.org/help"), false));
  EXPECT_TRUE(history.Committed(5, login, true));
}

TEST(AgentSignInRule, ANavigationThatEndedUncommittedIsNoLongerTheWithheldPages) {
  WithheldHistory history;
  history.Withhold(GURL("https://shop.example.org/login"), {7});
  history.Ended(7);
  EXPECT_FALSE(history.Committed(7, GURL("https://shop.example.org/help"), false));
}

TEST(AgentSignInRule, ASubmitterSendingAFormElsewhereIsFound) {
  EXPECT_TRUE(SendsElsewhere({{"formaction", "https://collector.example.com/"}}));
  EXPECT_TRUE(SendsElsewhere({{"formaction", "/session"}}));
  EXPECT_TRUE(SendsElsewhere({{"type", "submit"}, {"FORMTARGET", "_blank"}}));
  EXPECT_TRUE(SendsElsewhere({{"target", "results"}}));
  EXPECT_TRUE(SendsElsewhere({{"target", "_parent"}}));
  EXPECT_TRUE(SendsElsewhere({{"target", "_top"}}));
}

TEST(AgentSignInRule, ASubmitterKeepingTheFormsOwnWayIsNot) {
  EXPECT_FALSE(SendsElsewhere({}));
  EXPECT_FALSE(SendsElsewhere({{"formaction", ""}}));
  EXPECT_FALSE(SendsElsewhere({{"formaction", "  "}}));
  EXPECT_FALSE(SendsElsewhere({{"formtarget", "_self"}}));
  EXPECT_FALSE(SendsElsewhere({{"target", " _SELF "}}));
  EXPECT_FALSE(SendsElsewhere({{"target", ""}}));
  EXPECT_FALSE(SendsElsewhere({{"action", "https://collector.example.com/"}}));
}

TEST(AgentSignInRule, TheSubmittersSelectorNamesEachWay) {
  const std::string_view selector = kSubmitters;
  for (const char* each : {"[formaction]", "[formtarget]", "form[target]", "base[target]"}) {
    EXPECT_NE(selector.find(each), std::string_view::npos) << each;
  }
}

}  // namespace
}  // namespace browser_core
