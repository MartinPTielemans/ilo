# Signed-in pages on public demos, 2026-11-10

Whether the signed-in pages of a bank and of health records send the signals the memory keeps
pages out by: Cache-Control: no-store (the page alone), and schema.org JSON-LD (the whole site).
Read with curl from quits-runner-1, each demo's own cookies, made-up data only.

- ParaBank (parabank.parasoft.com), Parasoft's demo bank: an account registered with made-up
  details for this, signed in.
- OpenEMR 8.5.0 patient portal (demo.openemr.io/openemr/portal), signed in as the demo's own
  patient, with the credentials open-emr.org/demo publishes. A patient's view of their records.
- OpenMRS O3 (dev3.openmrs.org), signed in with the credentials openmrs.org/demo publishes. A
  clinician's view of a made-up patient's chart.

No public insurance demo with a customer sign-in turned up.

## ParaBank: 7 of its 8 account pages send no-store

| Page | Cache-Control | JSON-LD |
|---|---|---|
| overview.htm | none | none |
| openaccount.htm | no-store | none |
| transfer.htm | no-store | none |
| billpay.htm | no-store | none |
| findtrans.htm | no-store | none |
| updateprofile.htm | no-store | none |
| requestloan.htm | no-store | none |
| index.htm | none | none |
| about.htm | none | none |
| activity.htm | no-store | none |

overview.htm, Accounts Overview, is the page with every account's balance, and it sends no
Cache-Control at all. index.htm and about.htm, the same while signed in, send none either.

## OpenEMR patient portal: 0 of 6 signed-in pages send no-store

| Page | Cache-Control | JSON-LD |
|---|---|---|
| home.php | max-age=0, private, must-revalidate | none |
| messaging/messages.php | max-age=0, private, must-revalidate | none |
| report/portal_patient_report.php | max-age=0, private, must-revalidate | none |
| report/pat_ledger.php | max-age=0, private, must-revalidate | none |
| patient/onsitedocuments | max-age=0, private, must-revalidate | none |
| ccda_gateway.php | max-age=0, private, must-revalidate | none |

The fragments home.php loads (get_lab_results.php, get_medications.php, get_problems.php,
get_profile.php) send the same.

## OpenMRS O3: 0 of 4 signed-in chart pages send no-store

| Page | Cache-Control | JSON-LD |
|---|---|---|
| spa/home | no-cache, must-revalidate | none |
| spa/patient/<patient>/chart/Patient%20Summary | no-cache, must-revalidate | none |
| spa/patient/<patient>/chart/Results | no-cache, must-revalidate | none |
| spa/patient/<patient>/chart/Medications | no-cache, must-revalidate | none |
