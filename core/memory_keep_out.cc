#include "browser_core/memory_keep_out.h"

#include <algorithm>
#include <array>

#include "base/containers/span.h"
#include "base/strings/string_util.h"

namespace browser_core {

namespace {

// schema.org's types for money and health, as its vocabulary names them, not sites: a page says
// which it is. Organization and LocalBusiness types say the whole site is one; the Medical*
// types for content say the page is about health, and a site that publishes one keeps the rest of
// its pages out too, as its reader's other pages there are likely health reading as well.
constexpr auto kBank = std::to_array<std::string_view>({
    "BankAccount", "BankOrCreditUnion", "BrokerageAccount", "CreditCard", "CurrencyConversionService",
    "DepositAccount", "FinancialProduct", "FinancialService", "InsuranceAgency", "InvestmentFund",
    "InvestmentOrDeposit", "LoanOrCredit", "MortgageLoan", "PaymentCard", "PaymentService",
});

constexpr auto kHealth = std::to_array<std::string_view>({
    "Dentist", "DiagnosticLab", "Drug", "DrugClass", "Hospital", "MedicalBusiness", "MedicalClinic",
    "MedicalCondition", "MedicalEntity", "MedicalOrganization", "MedicalProcedure",
    "MedicalScholarlyArticle", "MedicalSignOrSymptom", "MedicalStudy", "MedicalTest",
    "MedicalTherapy", "MedicalWebPage", "Optician", "Patient", "Pharmacy", "Physician",
    "PhysiciansOffice", "PsychologicalTreatment",
});

// A type as a page may write it: "BankOrCreditUnion", "schema:BankOrCreditUnion" or
// "https://schema.org/BankOrCreditUnion".
std::string_view Bare(std::string_view type) {
  size_t cut = type.find_last_of("/:#");
  return cut == std::string_view::npos ? type : type.substr(cut + 1);
}

bool Has(base::span<const std::string_view> names, std::string_view type) {
  return std::ranges::find(names, Bare(type)) != names.end();
}

}  // namespace

std::optional<KeptOut> MemorySiteSays(std::string_view site, const std::vector<std::string>& types) {
  for (const std::string& type : types) {
    if (Has(kBank, type)) {
      return KeptOut::kBank;
    }
    if (Has(kHealth, type)) {
      return KeptOut::kHealth;
    }
  }
  if (base::EndsWith(site, ".bank") || base::EndsWith(site, ".insurance")) {
    return KeptOut::kDomain;
  }
  return std::nullopt;
}

}  // namespace browser_core
