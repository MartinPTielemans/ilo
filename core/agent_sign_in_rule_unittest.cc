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

}  // namespace
}  // namespace browser_core
