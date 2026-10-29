# W3 — Data residency and mining-data export statutes (OPEN-B4-01)

RESULT_STATUS: PARTIAL_CLOSE — 72/72 `(country, data_class)` rows are registered, each with a cited basis. Status split: 10 FACT, 23 FACT[U], 39 INFERENCE. Eight named clauses still need counsel (§7). As of 2026-10-29.

> This is research for engineers and executives. It is not legal advice. Every row has a `review_by` date and an evidence ID, and the registry stays fail-closed for any pair not listed here.

## 1. Executive summary

1. **Pakistan (Reko Diq).** No personal-data statute is in force. The PDP Bill 2023 is still a draft, and its s.31(2) would keep "critical personal data" on servers in Pakistan (E511–E512). Reko Diq sits outside the new Balochistan Mines and Minerals Act 2025. It stays governed by its mineral agreement plus the Balochistan Mineral Rules 2002 (BMMA s.3(2), s.122(4); E501–E503).
2. **What BMR 2002 requires at Reko Diq.** Records are kept "at an address in Balochistan" (r.56; E504). The State has "exclusive right to all data", including geological and production data. Disclosure is allowed only "in connection with" mining operations or where the law requires it (r.71; E505–E506). Books are kept in Pakistan (Companies Act 2017 s.220; E508). **Verdict:** `personal` and `state_secret` are `in_country_only`; `geoscience`, `production` and `financial` are `aggregates_exportable`; `aggregate` is `exportable`.
3. **Zambia (Lumwana, M2 pilot).** Zambia has a hard localisation law: "A data controller shall process and store personal data on a server or data centre located in the Republic" (DPA 2021 s.70(1); E514). Sensitive data, which by definition includes sex, biometric and health data, must also be processed and stored in Zambia (s.70(3), E515–E517).
4. **Zambia, cyber law.** The Cyber Security Act 2025 names mining as a critical sector (s.8(j)). Once the Agency designates information by Gazette notice, it must be hosted in Zambia (s.12). No mining designation was found as of 2026-10-29 (E520–E522). **Impact on the M2 pilot:** Muster/personal data needs an in-Zambia regional node from day one. Rollups can replicate.
5. **Tanzania.** The Mining Act s.35(6) bars export of "any core, cuttings, rock samples, soil, fluid samples or any other data collected" without written authorisation from the Geological Survey of Tanzania (GST) (E527). Personal-data transfers need a PDPC permit (Regs 2023 reg. 20; E530). Tax documents must be maintained "within the United Republic" (E531).
6. **DRC and Mali.** In the DRC, personal data "sont stockées et/ou hébergées en République Démocratique du Congo". Transfers need prior authorisation from the data-protection authority (APD), whose powers are exercised for now by the telecom regulator ARPTC (Code du numérique art. 201; E532–E533). Mali allows transfers only on an adequacy finding or a decision of its data-protection authority, APDP (Loi 2013-015 art. 11; E537). Its mining code requires sample assays to be done in Mali unless authorised in writing (art. 24; E538).
7. **PNG.** No data-protection law is in force. The DGDP Bill 2026 (5th draft) would prohibit storing raw biometric data outside PNG "under any circumstances" (s.60(4)). It would also create transfer controls (s.81) and ministerially designated "strategic significance" hosting (s.82) (E540). The Mining Act 1992 has confidentiality rules but no rule on where data must be located (E539).
8. **Chile.** Ley 21.719 takes effect on **2026-12-01** (E542). Personal-data transfers will then need adequacy, model clauses (approved 2025-12-11; E543) or binding corporate rules (BCR). Geological data must be submitted to Sernageomin, but there is no localisation rule (E544).
9. **Rest of the portfolio.**
   - Argentina, Dominican Republic and Saudi Arabia: personal data stays in-country until a transfer instrument is in place.
   - USA: no localisation, except for bulk sensitive data sent to "countries of concern" under 28 CFR 202.
   - Canada: books must be kept in Canada (ITA s.230(1)).
   - Everything else in these six countries is research-grade INFERENCE.
10. **Schema gap.** Three values cannot express two common legal shapes:
    - "the authoritative copy stays in-country, replicas are allowed" (most book-keeping statutes);
    - "exportable once instrument X exists" (most data-protection laws).
    
    We encode both conservatively and propose optional `requires` and `deny_destinations` fields (§4).

### Decision rules used (so a reviewer can re-derive every row)

- **R1 → `in_country_only` (FACT).** A provision in force bans export, or makes it conditional on an authorisation or permit we cannot assume has been obtained.
- **R2 → `aggregates_exportable`.** A provision requires records or books to be kept in-country but does not ban copies abroad, so raw or authoritative data stays and rollups move. The row is FACT when the location provision was read in primary text, and INFERENCE when applying it to this class needs interpretation.
- **R3 → `aggregates_exportable` (INFERENCE).** The law is silent, but the State owns the minerals or data, has project equity, or there is a pending designation or bill. This is fail-closed for raw data.
- **R4 → `exportable`.** We read the relevant primary texts and found no location or export rule, or there is a positive transfer framework with no localisation. Also used for anonymised aggregates that no statute reaches.
- **`state_secret` is always `in_country_only`**, by definition of the class.

## 2. Source register

Local paths are relative to `/private/tmp/claude-501/-Users-jemanuel-projects-franken-mining/2ba72fbb-c45d-4fd8-93cf-c08cb26d7a18/scratchpad/w3/`. "URL n/r" means the prior run did not record the download URL; the file content was checked in this run. Access date for all sources is 2026-10-29.

| ID | Country | Instrument (number, date) | URL | Local file | Kind |
|---|---|---|---|---|---|
| S01 | PK | Balochistan Mines and Minerals Act 2025, Act XIII of 2025. Passed 12 Mar, assented 14 Mar, notified 17 Mar 2025 | URL n/r (Balochistan PA Secretariat notification PAB/Legis:V(13)/2025/4770) | `pk/bmma2025.pdf` (scan); OCR in `pk/ocrtxt/all.txt` (tesseract, this run) | Primary (gazette scan; OCR typos normalised in quotes) |
| S02 | PK | Balochistan Mineral Rules 2002, 2nd ed. Nov 2008 | URL n/r | `pk/bmr2002.pdf`, `.txt` | Primary (consolidated edition; post-2008 amendments **not** checked) |
| S03 | PK | Companies Act 2017 (Act XIX of 2017), Gazette 31 May 2017 | https://download1.fbr.gov.pk/Docs/202010512102039492CompaniesAct,2017.pdf | `pk/ca2017.pdf`, `.txt` | Primary (as enacted) |
| S04 | PK | Prevention of Electronic Crimes Act 2016, as passed | URL n/r | `pk/peca2016.pdf`, `pk/peca.txt` (OCR) | Primary (2025 amendments not reviewed) |
| S05 | PK | Pakistan Cloud First Policy, MoITT, Feb 2022 | URL n/r | `pk/cloudfirst.pdf`, `.txt` | Primary (policy, not statute) |
| S06 | PK | Digital Rights Foundation table on the MoITT Personal Data Protection Bill (2023 draft) | URL n/r | `pk/drf.pdf`, `.txt` | Secondary (quotes bill text) |
| S07 | PK | Chambers *Data Protection & Privacy 2026 — Pakistan* (S.U.Khan Associates), updated 10 Mar 2026 | https://practiceguides.chambers.com/practice-guides/data-protection-privacy-2026/pakistan | — | Secondary |
| S08 | ZM | Data Protection Act 2021, Act No. 3 of 2021 | https://www.parliament.gov.zm/sites/default/files/documents/acts/Act%20No.%203%20The%20Data%20Protection%20Act%202021_0.pdf | `zm/dpa2021.pdf`, `.txt` | Primary |
| S09 | ZM | SI No. 22 of 2021, DPA (Commencement) Order, 31 Mar 2021 | URL n/r | `zm/si22_2021.pdf`, `zm/si22.txt` | Primary |
| S10 | ZM | Cyber Security Act 2025, Act 3 of 2025 (assent 8 Apr 2025) | https://media.zambialii.org/media/legislation/44117/source_file/1295e757d288ef63/2025-3.pdf | `zm/csa2025.pdf`, `.txt` | Primary (Laws.Africa consolidation) |
| S11 | ZM | Cyber Crimes Act 2025, Act 4 of 2025 | URL n/r (ZambiaLII) | `zm/cca2025.pdf`, `.txt` | Primary |
| S12 | ZM | Minerals Regulation Commission Act 2024, Act 14 of 2024 | https://media.zambialii.org/media/legislation/43999/source_file/8159be48e21c2cc1/2024-14.pdf | `zm/mrca2024.pdf`, `.txt` | Primary (version of 26 Dec 2024, then "not commenced") |
| S13 | ZM | Geological and Minerals Development Act 2025, Act 2 of 2025 | https://www.parliament.gov.zm/sites/default/files/documents/acts/Acts%20No.%202%20of%202025%2C%20The%20Geological%20Minerals%20Development.pdf | `zm/gmda2025_b.pdf`, `.txt` | Primary |
| S14 | ZM | SI 42 of 2025, MRC Act (Commencement) Order (in force 3 Jun 2025) | https://zambialii.org/akn/zm/act/si/2025/42 | — (HTTP 403) | Secondary (listing metadata) |
| S15 | TZ | Personal Data Protection Act 2022 (English text, GN 395B, 13 Jun 2023) | URL n/r (pdpc.go.tz) | `tz/pdpa2022.pdf`, `.txt` | Primary |
| S16 | TZ | PDP (Personal Data Collection and Processing) Regulations 2023, GN 449C, 4 Jul 2023 | URL n/r (pdpc.go.tz) | `tz/pdp_regs2023.pdf`, `tz/regs2023.txt` | Primary |
| S17 | TZ | Mining Act Cap 123 R.E. 2023 | https://www.tumemadini.go.tz/media/uploads/publications/2025/06/29/The_Mining_Act.pdf | `tz/mining_act.pdf`, `.txt` | Primary (watermarked OAG copy) |
| S18 | TZ | Tax Administration Act Cap 438 R.E. 2019 | https://www.mof.go.tz/uploads/documents/en-1676548563-THE%20TAX%20ADMINISTRATION_ACT,%20CAP%20438.pdf | `tz/taa438.pdf`, `.txt` | Primary |
| S19 | CD | Code du numérique, Ordonnance-loi of 13 Mar 2023. Number 23/010 per secondary citation; the text shows only the date | URL n/r (droitnumerique.cd) | `cd/code_numerique.pdf`, `.txt` | Primary text, unofficial reproduction |
| S20 | CD | Loi n°18/001 du 9 mars 2018 modifiant la Loi 007/2002 (Code minier) | URL n/r (leganet.cd) | `cd/loi18001.html`, `.txt` | Primary (unofficial) |
| S21 | CD | Loi n°007/2002 du 11 juillet 2002 portant Code minier | URL n/r | `cd/code_minier_2002.pdf`, `.txt` | Primary (pre-2018 text) |
| S22 | CD | B. Kandolo wa Kandolo, droitnumerique.cd *Dossier n°11*, May 2026 (two papers on ARPTC decision of 11 Sep 2025) | URL n/r | `cd/illegalite.pdf`, `cd/inconst.pdf` (+ `.txt`) | Secondary |
| S23 | ML | Loi n°2013-015 du 21 mai 2013 (protection des données à caractère personnel) | https://amrtp-mali.com/wp-content/uploads/2025/08/LOI-2013-015.pdf | `ml/loi2013015.pdf`, `.txt` | Primary |
| S24 | ML | Loi n°2023-040 du 29 août 2023 portant Code minier (JO) | https://faolex.fao.org/docs/pdf/mli219801.pdf | `ml/cmin2023.pdf`, `ml/cmin2023_flow.txt` | Primary |
| S25 | PG | Mining Act 1992 | https://mra.gov.pg/wp-content/uploads/2021/08/2.MINING_ACT-1992.pdf | `pg/mining_act_1992.pdf`, `.txt` | Primary |
| S26 | PG | Mineral Resources Authority Act 2005 | https://mra.gov.pg/wp-content/uploads/2021/08/4.MRA-Act-2005_Certified.pdf | `pg/mra_act_2005.pdf` (scan, not text-searched) | Primary |
| S27 | PG | Government Cloud Policy 2023 v2.7 | https://www.ict.gov.pg/Policies/Cloud%20Policy/Final%20Government%20Cloud%20Policy%202023%20V2.7%20-%20Final.pdf | `pg/cloudpolicy.pdf`, `.txt` | Primary (policy) |
| S28 | PG | DGDP Bill 2026 consultation notice (closes 19 Aug 2026) | https://www.ict.gov.pg/Draft%20Bill/Data_Governance_and_Data_Protection_Bill_2026_Consultation_Notice.pdf | `pg/notice.pdf`, `.txt` | Primary (draft) |
| S29 | PG | DGDP Act 2026, proposed 5th draft | https://www.ict.gov.pg/Draft%20Bill/DGDP%20Act%202026%20Fifth%20Draft_PublicConsultation.pdf | `pg/dgdp2026_draft5.pdf`, `.txt` | Primary (bill) |
| S30 | CL | Ley 21.719 (D.O. 44.023, published 2024-12-13) | LeyChile/BCN XML (URL n/r) | `cl/l21719.xml`, `.txt` | Primary |
| S31 | CL | Ley 19.628, version effective 2026-12-01 | LeyChile/BCN XML (URL n/r) | `cl/l19628_v2026.xml`, `.txt` | Primary |
| S32 | CL | Res. Exenta RAEX202503748, Min. Economía, 11 Dec 2025 (model clauses) | LeyChile/BCN XML (URL n/r) | `cl/cct.xml`, `.txt` | Primary |
| S33 | CL | Decreto 9/2024, Min. Minería (geological information reporting rule), 26 Apr 2024 | LeyChile/BCN XML (URL n/r) | `cl/ds9.xml`, `.txt` | Primary |
| S34 | CL | Ley 21.663, Ley Marco de Ciberseguridad | LeyChile/BCN XML (URL n/r) | `cl/l21663.xml`, `.txt` | Primary |
| S35 | CL | DL 830, Código Tributario | LeyChile/BCN XML (URL n/r) | `cl/dl830.xml`, `.txt` | Primary |
| S36 | AR | Ley 25.326 art. 12; Disp. DNPDP 60-E/2016; Res. AAIP 34/2019 | https://www.argentina.gob.ar/normativa/nacional/267922/texto | — | Secondary (search summary; primary not downloaded) |
| S37 | DO | Ley 172-13 (promulgated 13 Dec 2013) | https://phlaw.com/ley-172-13-sobre-proteccion-de-datos-personales/ | — | Secondary (article number not verified) |
| S38 | SA | NCA Essential Cybersecurity Controls ECC-2:2024 (English; Arabic is binding) | https://cdn.nca.gov.sa/api/files/public/upload/86e09090-44e4-481f-bc28-355673607654_ECC--2024-EN.pdf | `sa/ecc2_2024.pdf`, `.txt` | Primary |
| S39 | SA | PDPL (Royal Decree M/19, amended) art. 29; Transfer Regulation amended Sep 2024; SDAIA SCCs | https://www.mayerbrown.com/ja/insights/publications/2024/10/updates-to-saudi-arabias-personal-data-protection-regulations-sccs-guidelines-and-more | — | Secondary |
| S40 | US | 28 CFR Part 202 (DOJ Data Security Program; EO 14117), effective 2025-04-08 | https://www.debevoise.com/insights/publications/2025/05/maturing-compliance-with-the-bulk-sensitive-data | — | Secondary |
| S41 | CA | Income Tax Act s.230 (current to 2026-09-30) | https://laws-lois.justice.gc.ca/eng/acts/I-3.3/section-230.html | — (fetched online) | Primary |

**Downloaded but not relied on:** `cl/cmin.xml` (Código de Minería), `cl/l21420.xml`, `pg/mra_regs.html`, `pg/paclii_ma.html`, `cd/refworld.html`, `cd/leganet_index.html`, `zm/si21_2025.pdf` (Gazette of 9 May 2025, no relevant notice), `pk/dps_277911.pdf` (a PSX Form 21 filing). Two files are failed downloads: `zm/si22_2025.pdf` and `zm/si23_2025.pdf` are 404 HTML.

## 3. Country findings

Quotes are verbatim. OCR typos in S01 and S04 are normalised and marked with [ ].

### PK — Pakistan (Reko Diq). Top priority.

**Which regime governs Reko Diq**
- **BMMA 2025 s.3(2):** "The provisions of this Act shall not apply to any licence, lease, permit, mineral agreement, Negotiated Agreement or Qualified Investment as specified in sub-Section (4) of Section 122."
- **s.4, "Qualified Investment":** "...specifically includes the Reko Diq project in the Province, as described in the First Schedule to the Foreign Investment (Promotion and Protection) Act, 2022 (XXXV of 2022)..."
- **s.122(4):** such agreements "shall remain unaffected by this Act in all respects and shall continue to be governed by, subject to and construed by reference to, the existing terms and conditions of such ... mineral agreement or Negotiated Agreement, as applicable, together with the necessary enabling legislation, the applicable provisions of the [repealed laws and the] Balochistan Mineral Rules, 2002 and the terms of the applicable mineral agreement..."
- **Status:** in force ("shall come into operation at once", s.2).
- **Consequence:** the BMMA's own data clauses (s.18 geological database, s.116 confidentiality) do **not** reach Reko Diq. BMR 2002 and the non-public mineral agreement do.

**Geoscience and production data (BMR 2002)**
- **r.56(1):** "The lessee shall prepare and keep at an address in Balochistan, in a form acceptable to the licensing authority, proper records of— (a) the mining operations..."
  - The listed items include tonnages won and sold, waste, geology, drilling and sampling results, "analytical, metallurgical and mineralogical work", interpretations, and capital and operating expenditure.
  - The same wording appears for the other title types: r.22, r.33, r.44, r.81.
- **r.71(1):** "the Government shall have the exclusive right to all data including geological, geophysical, geochemical, petrophysical, engineering, pit logs, maps, magnetic tapes, cores and production data, as well as all interpretative and derivative data including reports, studies, analysis, interpretations, bulk sampling results, assaying results, evaluations and other information in respect of exploration or mining operations."
- **r.71(2):** the holder may use the data "free of cost" and "retain copies or samples".
- **r.71(3):** such data "which is not in the public domain, shall not be disclosed to any person without the prior consent of the licensing authority, except— (i) as may be necessary for the purpose of, or in connection with, exploration and mining operations; or (ii) as required by law, or for the purpose of arbitration or litigation."
- **Status:** in force (2008 consolidated edition read; later amendments not checked).
- **Implication:**
  - Raw geoscience and production records must have an authoritative copy in Balochistan.
  - Copies abroad are not banned outright. Disclosure to Barrick group entities or a cloud provider depends on reading r.71(3)(i) ("in connection with ... mining operations").
  - Fail-closed encoding: `aggregates_exportable`.
  - Aggregates that Barrick must publish fall under r.71(3)(ii) ("required by law": securities disclosure). Hence `aggregate` = `exportable`.

**Financial (Companies Act 2017 s.220)**
- **s.220(1):** "Every company shall prepare and keep at its registered office books of account and other relevant books and papers and financial statements for every financial year..."
- **Second proviso:** the books "may be kept at such other place in Pakistan as the board may decide."
- **s.220(3):** "...in the case of financial information, if any, maintained outside the country, copies of such financial information shall be maintained and produced for inspection by any director."
- **First proviso:** mining companies must also keep cost particulars.
- **Status:** in force.
- **Implication:** books stay in Pakistan, and the Act itself contemplates financial information also held abroad. Encoded as `aggregates_exportable` (FACT). This assumes RDMC is Pakistan-incorporated, which we did not verify (§7 Q1).

**Personal data**
- No statute in force. Chambers 2026: the Draft Bill "has yet to be passed by both houses of the Parliament" and Pakistan "does not yet have an operative data protection law in force" (S07).
- PECA 2016 is a criminal statute with no localisation clause.
- **Draft bill s.31(2)** (2023 text via S06): "Critical Personal Data shall only be processed in a server(s) or digital infrastructure located within the territory of Pakistan."
- **Draft s.2(g)** defines "critical personal data" loosely, by Commission classification.
- **Status:** BILL (not enacted as of 2026-10-29).
- **Implication:** `in_country_only` (INFERENCE). Grounds:
  - the class includes biometric and health data, which the draft treats as sensitive;
  - the draft would localise data on classification;
  - the Cloud First Policy treats PII as "Sensitive/Confidential", hosted on Government or private cloud located in Pakistan (Annex D and E). That binds only federal public-sector entities, and serves only as "guidance" to private ones.

**State secret / critical infrastructure**
- **PECA s.2(j):** the Government "may designate any private or Government infrastructure ... as critical infrastructure".
- **PECA s.7:** "Whoever with dishonest intention and without authorization copies or otherwise transmits or causes to be transmitted any critical infrastructure data shall be punished with imprisonment for a term which may extend to five years..."
- Official Secrets Act 1923: not fetched, so FACT[U].
- **Implication:** `in_country_only`. Whether Reko Diq has been designated as critical infrastructure is unknown (§7).

**Verdict PK:**

| personal | geoscience | production | financial | state_secret | aggregate |
|---|---|---|---|---|---|
| in_country_only | aggregates_exportable | aggregates_exportable | aggregates_exportable | in_country_only | exportable |

### ZM — Zambia (Lumwana, M2 pilot site). Second priority.

**Personal data (DPA 2021, Act No. 3 of 2021; in force by SI 22 of 2021)**
- **s.70(1):** "A data controller shall process and store personal data on a server or data centre located in the Republic."
- **s.70(2):** "Despite subsection (1), the Minister may prescribe categories of personal data that may be stored outside the Republic."
- **s.70(3):** "Despite subsection (2), sensitive personal data shall be processed and stored in a server or data centre located in the Republic."
- **s.71(1):** other personal data "may be transferred outside the Republic where— (a) the data subject has consented and (i) the transfer is made subject to standard contracts or intragroup schemes that have been approved by the Data Protection Commissioner; or (ii) the Minister, has prescribed that transfers outside the Republic is permissible; or (b) the Data Protection Commissioner approves a particular transfer or set of transfers as permissible due to a situation of necessity."
- **s.71(4)(b):** transfer is allowed "where the data subject has explicitly consented to that transfer of sensitive personal data".
- **Definition of "sensitive personal data"** includes "(a) the race, marital status, ethnic origin or sex of a data subject; (b) genetic data and biometric data; ... (g) a data subject's physical or mental health". That covers essentially every HR, fatigue or medical record.
- **Status:** in force. No s.70(2) statutory instrument and no ODPC-approved intragroup scheme was found (searched 2026-10-29).
- **Implication:**
  - `personal` = `in_country_only` (FACT, R1).
  - Muster pseudonymisation does not change this: pseudonymised data is still personal data.

**Critical information (Cyber Security Act 2025, Act 3 of 2025)**
- **s.8:** "a critical sector includes— ... (j) mining".
- **s.9(1):** the Agency "shall, by notice in the Gazette designate information or information infrastructure relevant to a critical sector as critical information or critical information infrastructure".
- **s.12(1):** "A controller shall host critical information or critical information infrastructure within the Republic, in a prescribed manner and form."
- **s.12(2):** the Agency may authorise hosting outside. s.12(3)(h) lists "the categories of personal data required to be stored within the Republic under the Data Protection Act, 2021" as a factor.
- **Definitions:** "controller" means a person responsible for CI/CII "that is registered under this Act". "Critical information" includes "(c) information needed to operate critical information infrastructure".
- **Status:** in force since 12 May 2025 (S10 metadata). No Gazette designation of mining information was found.
- **Implication:** Lumwana operational data is one Gazette notice away from mandatory in-country hosting. `production` and `geoscience` therefore stay `aggregates_exportable`, not `exportable`. The registry should watch the Gazette (review_by 2027-01-31).

**Mining (MRC Act 2024 and GMDA 2025; both in force 3 Jun 2025 per S14)**
- **MRC Act s.12(2)(c):** a mining right "shall not be granted to or held by a company which— ... (c) has not established a registered office in the Republic".
- **Definition:** "registered office" means an office "for the delivery of correspondence, service of notices or documents and for the storage of records as provided under this Act".
- **s.54:** on termination, all records must be delivered to the Commission.
- **GMDA s.11:** restricts physical removal of minerals and samples.
- No digital-data export ban was found.
- **Implication:** records are stored in-country (R2/R3), so `geoscience` and `production` are `aggregates_exportable`.

**Financial**
- We could not read the Companies Act 2017 (Act No. 10 of 2017) section on accounting records: WIPO Lex and ZambiaLII returned 403.
- Encoded `aggregates_exportable`, FACT[U].

**Verdict ZM:**

| personal | geoscience | production | financial | state_secret | aggregate |
|---|---|---|---|---|---|
| in_country_only | aggregates_exportable | aggregates_exportable | aggregates_exportable | in_country_only | exportable |

### TZ — Tanzania (North Mara, Bulyanhulu; Twiga Minerals)

**Mining data (Mining Act Cap 123 R.E. 2023)**
- **s.35(2):** "All mineral data generated under this Act shall be owned by the Government."
- **s.35(3):** the holder "shall submit to the Geological Survey of Tanzania" maps, "geophysical and geochemical raw data", "processed and interpreted data or maps", technical reports, and "core samples and its mineral exploration data base".
- **s.35(6):** "The mineral right holder shall not export any core, cuttings, rock samples, soil, fluid samples or any other data collected without the written authorisation of the Geological Survey of Tanzania."
- **s.125(1):** "The holder of a mineral right shall keep records within the mining areas or at the mineral rights holders' office located in Tanzania for as long as the mineral right subsists..."
- **Status:** in force.
- **Implication:**
  - `geoscience` = `in_country_only` (FACT, R1). Add a GST authorisation as `requires`.
  - `production` = `aggregates_exportable` (INFERENCE). Whether "any other data collected" reaches production telemetry is a counsel question (§7 Q3).

**Personal data**
- **PDPA 2022 s.31(1):** "The Commission may, subject to the provisions of this Act, prohibit the transfer of personal data to a place outside the country."
- **s.32(4):** derogations, including consent and contract necessity.
- **Regs 2023 reg. 20(1):** "A data controller or data processor who intends to transfer personal data outside the country, shall submit an application for permit to the Commission using Form No. 7".
- **Reg. 21(c):** a permit may be refused if "the transfer of personal data is restricted by other written laws".
- **Reg. 22:** the permit is limited to the recipient and purpose named in it.
- **Status:** in force.
- **Implication:** `in_country_only` (FACT, R1) until a PDPC permit exists.

**Financial**
- **Tax Administration Act s.35(1):** "Every taxable or liable person shall, within the United Republic, maintain documents in paper or electronic form which— (a) contain information to be provided or filed with the Commissioner General under any tax law; (b) enable an accurate determination of tax payable..."
- **Implication:** `aggregates_exportable` (FACT, R2).

**Verdict TZ:**

| personal | geoscience | production | financial | state_secret | aggregate |
|---|---|---|---|---|---|
| in_country_only | in_country_only | aggregates_exportable | aggregates_exportable | in_country_only | exportable |

### CD — DRC (Kibali)

**Personal data (Code du numérique, ordonnance-loi of 13 Mar 2023)**
- **Art. 201:** "Les données personnelles sont stockées et/ou hébergées en République Démocratique du Congo."
- Transfers to a third state are possible only when the APD "constate que l'État ... assure un niveau de protection adéquat et suffisant".
- "Avant tout transfert effectif de données à caractère personnel vers un État tiers ou une organisation internationale, le responsable du traitement doit préalablement obtenir l'autorisation de l'Autorité de protection des données".
- **Art. 390:** entry into force on promulgation.
- **Status:** in force.
  - The APD has not yet been set up. ARPTC exercises its powers "à titre provisoire".
  - ARPTC's decision of 11 Sep 2025 sets the declaration and authorisation procedure, including transfer conditions (S22, secondary). Its legality is contested in that commentary.
- **Implication:** `in_country_only` (FACT, R1).

**Mining (Code minier 2002 as amended by Loi 18/001)**
- **Art. 50:** "Tout échantillon prélevé dans le cadre du Permis de recherches est propriété de l'Etat." Export of samples needs the Direction de géologie's visa.
- **Art. 216 (2018):** holders must "tenir les registres, d'élaborer et de déposer les rapports ... conformément au Règlement minier", and publish monthly "les quantités produites, vendues ou exportées".
- **Art. 324 (2002 text):** technical and geological information supplied "sont confidentiels pour une durée de dix ans".
- No digital-data location rule was found. We did not read the Règlement minier (Décret 038/2003 as amended).
- **Implication:** `geoscience` and `production` = `aggregates_exportable` (INFERENCE, R3). Production aggregates are published monthly by law.

**Verdict CD:**

| personal | geoscience | production | financial | state_secret | aggregate |
|---|---|---|---|---|---|
| in_country_only | aggregates_exportable | aggregates_exportable | aggregates_exportable | in_country_only | exportable |

### ML — Mali (Loulo-Gounkoto)

**Personal data**
- **Loi 2013-015 art. 11:** "Le responsable d'un traitement peut transférer des données personnelles vers un Etat étranger : lorsque l'Etat destinataire assure un niveau suffisant de protection ... constaté par l'Autorité ...; par décision de l'Autorité ..., lorsque le transfert ... garanti[t] un niveau de protection suffisant ..., notamment en raison des clauses contractuelles ou règles internes dont il fait l'objet."
- The APDP's listed functions include "autoriser le transfert de données" (`ml/loi2013015.txt` l.572).
- **Status:** in force (amended 2017 per secondary sources; amendment not read).
- **Implication:** `in_country_only` (FACT, R1) until there is an APDP decision.

**Mining (Loi 2023-040, Code minier)**
- **Art. 24:** "Les analyses d'échantillons sont effectuées au Mali. Toutefois, le titulaire d'un titre minier peut, après justification, effectuer des analyses d'échantillons en dehors du Mali sur autorisation écrite de l'administration chargée des Mines."
- **Art. 194:** a register on every site.
- **Art. 195:** confidentiality of results submitted to the administration.
- No digital-data export rule was found.
- **Status:** in force. Whether Loulo-Gounkoto is under the 2023 Code or its earlier convention, after the 2025 dispute and settlement, was **not verified** (§7).
- **Implication:** `geoscience` and `production` = `aggregates_exportable` (INFERENCE, R3).

**Verdict ML:**

| personal | geoscience | production | financial | state_secret | aggregate |
|---|---|---|---|---|---|
| in_country_only | aggregates_exportable | aggregates_exportable | aggregates_exportable | in_country_only | exportable |

### PG — Papua New Guinea (Porgera; New Porgera Ltd)

**Mining (Mining Act 1992)**
- **s.161:** the Director may require information on geology and mineral resources.
- **s.162:** "The holder of a tenement shall preserve all cores and drilling samples..."
- **s.163:** confidentiality of information disclosed to the Department.
- No data-location rule.
- **Status:** in force.

**Personal data**
- No statute in force. The DGDP Bill 2026 is a **draft**: 5th version, consultation closed 19 Aug 2026, no passage found.
- **Draft s.81(1):** "A data controller or data processor must not transfer personal data outside Papua New Guinea unless— (a) [adequacy]; (b) the transfer is authorised by the Authority with appropriate safeguards...; (c) the data subject has expressly consented...; or (d) the transfer is necessary for the performance of a contract with the data subject."
- **Draft s.60(4):** "Biometric raw data ... and root cryptographic key material used to secure any system processing biometric data, must not be stored or processed outside Papua New Guinea under any circumstances, including under an arrangement approved under Section 82."
- **Draft s.82(1)(c):** "data assets designated as of strategic significance by the Minister by notice in the National Gazette" must be stored and processed in PNG or in approved sovereign environments.
- **Implication:**
  - `personal` = `in_country_only` (INFERENCE). Mine access control and fatigue systems use biometrics.
  - `geoscience` and `production` = `aggregates_exportable`. Two reasons: the s.82 designation risk, and New Porgera's 51% PNG state and landowner ownership.
- The Government Cloud Policy 2023 applies only to "public bodies ... with the exception of state-owned entities". It does not bind New Porgera.

**Verdict PG:**

| personal | geoscience | production | financial | state_secret | aggregate |
|---|---|---|---|---|---|
| in_country_only | aggregates_exportable | aggregates_exportable | aggregates_exportable | in_country_only | exportable |

### Brief countries

**DO — Dominican Republic (Pueblo Viejo)**
- Ley 172-13 (13 Dec 2013) allows transfers abroad only with consent or listed exceptions such as contract performance, treaties, or judicial and medical cooperation (S37, secondary; article number not verified). → `personal` = `in_country_only`, FACT[U].
- Ley Minera 146-71, the Pueblo Viejo Special Lease Agreement and the Tax Code were not read. → Other classes are R3.

**AR — Argentina (Veladero)**
- Ley 25.326 art. 12 bans transfers to countries without adequate protection.
- Disp. 60-E/2016 art. 3 (as replaced by Res. AAIP 34/2019) lists **Canada (private sector)** as adequate, and model clauses exist (S36, secondary).
- → `personal` = `in_country_only`, with `allow_destinations = ["CA"]` proposed, FACT[U].
- Mining and accounting statutes were not read (R3).

**CL — Chile (Zaldívar)**
- **Ley 21.719, transitory art. 1:** the reform "entrarán en vigencia el día primero del mes vigésimo cuarto posterior a la publicación de esta ley en el Diario Oficial". Published 2024-12-13, so in force **2026-12-01**.
- **New art. 27:** transfers are lawful on adequacy, or under "cláusulas contractuales, normas corporativas vinculantes, u otros instrumentos jurídicos ... [con] garantías adecuadas", among other grounds.
- Model clauses were approved by Res. Ex. RAEX202503748 (2025-12-11). → `personal` = `in_country_only` until clauses are signed (FACT; review 2026-12-01).
- DS 9/2024 requires geological information to be **submitted** to Sernageomin. Neither DS 9/2024, Ley 21.663 nor Código Tributario art. 17 has a location rule. → `geoscience`, `production` and `financial` = `exportable` (INFERENCE, R4).

**SA — Saudi Arabia (Jabal Sayid; Ma'aden Barrick Copper Co.)**
- PDPL art. 29 plus the Transfer Regulation (amended Sep 2024): SCCs, BCRs or certification (S39, secondary). → `personal` = `in_country_only`, FACT[U].
- **NCA ECC-2:2024 scope:** "government agencies ... and their affiliated companies and entities (inside and outside the kingdom), as well as all private sector entities owning, operating, or hosting Critical National Infrastructures".
- ECC-2:2024 **deleted** sub-control 4-2-3-3 ("Entity's information hosting and storage must be inside the Kingdom"). It notes: "Controls related to data localization have been transferred ... to the National Data Management Office (NDMO)" (S38).
- Whether the JV is a government-affiliated company (through Ma'aden) is a counsel question. → Other classes are R3.

**US — USA (Nevada)**
- No general localisation law. 28 CFR Part 202 (effective 2025-04-08) restricts bulk US sensitive personal data, including biometric, health and geolocation data, going to countries of concern: China incl. HK/Macau, Russia, Iran, North Korea, Cuba, Venezuela (S40, secondary).
- → `personal` = `exportable`, with `deny_destinations` proposed, FACT[U].
- The CLOUD Act (18 U.S.C. §2713) lets US authorities compel US providers to produce data stored abroad. That matters for "sovereign hosting" tests elsewhere (PG draft s.82(2)(a)).

**CA — Canada (head office)**
- **ITA s.230(1):** records and books must be kept "at the person's place of business or residence in Canada or at such other place as may be designated by the Minister". → `financial` = `aggregates_exportable` (FACT).
- PIPEDA has no localisation rule, and Ontario has no private-sector privacy statute. Quebec's private-sector Act s.17 requires a privacy impact assessment before personal information leaves Quebec (not fetched). → `personal` = `exportable`, FACT[U].

## 4. Paste-ready `registries/residency.toml`

Rule values match the plan's three. Two kinds of fields here are **proposals**: `requires` (the instrument that lets an operator flip the row by a signed registry change), and `allow_destinations` / `deny_destinations`. Engineers may drop them; `rule` alone is the fail-closed contract.

```toml
# registries/residency.toml — source: planning/research/W3_data_residency.md (E501–E549), as of 2026-10-29.
# rule ∈ {in_country_only, aggregates_exportable, exportable}. Any pair not listed → in_country_only.
# status: FACT = primary text read and rule follows directly; FACT[U] = rests on secondary/unverified text;
#         INFERENCE = our reading where law is silent or ambiguous. Not legal advice.
schema = "fmine.residency/1"
residency_epoch = 1
as_of = "2026-10-29"
default_rule = "in_country_only"

# ---------------- PK — Pakistan (Reko Diq: mineral agreement + BMR 2002 per BMMA 2025 s.3(2), s.122(4)) ----------------
[[row]]
country = "PK"
data_class = "personal"
rule = "in_country_only"
basis = "No PD statute in force; PDP Bill 2023 (draft) s.31(2) localises critical personal data; Cloud First Policy 2022 Annex D/E"
status = "INFERENCE"
confidence = 0.60
review_by = "2027-01-31"
evidence = ["E510", "E511", "E512"]

[[row]]
country = "PK"
data_class = "geoscience"
rule = "aggregates_exportable"
basis = "BMR 2002 r.56(1) records kept at an address in Balochistan; r.71(1)-(3) State exclusive data right, disclosure only in connection with mining ops; BMMA 2025 s.122(4)"
status = "INFERENCE"
confidence = 0.55
review_by = "2027-01-31"
requires = "Counsel reading of r.71(3)(i) + Reko Diq mineral agreement data clause"
evidence = ["E502", "E503", "E504", "E505", "E506", "E513"]

[[row]]
country = "PK"
data_class = "production"
rule = "aggregates_exportable"
basis = "BMR 2002 r.56(1)(a) production/metallurgical records in Balochistan; r.71(1) 'production data' vested in Government"
status = "INFERENCE"
confidence = 0.55
review_by = "2027-01-31"
evidence = ["E504", "E505", "E506"]

[[row]]
country = "PK"
data_class = "financial"
rule = "aggregates_exportable"
basis = "Companies Act 2017 s.220(1) books at registered office or other place in Pakistan; s.220(3) copies of info held abroad; BMR r.56(1)(a)(ix)"
status = "FACT"
confidence = 0.75
review_by = "2027-04-30"
evidence = ["E508", "E504"]

[[row]]
country = "PK"
data_class = "state_secret"
rule = "in_country_only"
basis = "PECA 2016 s.2(j), s.7 (critical infrastructure data); Official Secrets Act 1923 (not fetched)"
status = "FACT[U]"
confidence = 0.90
review_by = "2027-04-30"
evidence = ["E509"]

[[row]]
country = "PK"
data_class = "aggregate"
rule = "exportable"
basis = "BMR 2002 r.71(3)(i)-(ii): disclosure in connection with mining ops or as required by law (issuer disclosure); anonymised KPIs not personal data"
status = "INFERENCE"
confidence = 0.60
review_by = "2027-01-31"
evidence = ["E506"]

# ---------------- ZM — Zambia (Lumwana; M2 pilot) ----------------
[[row]]
country = "ZM"
data_class = "personal"
rule = "in_country_only"
basis = "Data Protection Act 2021 s.70(1),(3); s.71 conditional transfers; sensitive incl. sex, biometric, health"
status = "FACT"
confidence = 0.90
review_by = "2027-01-31"
requires = "s.70(2) SI, or consent + ODPC-approved standard contract/intragroup scheme (s.71(1)(a)(i)); sensitive data: counsel on s.70(3) vs s.71(4)(b)"
evidence = ["E514", "E515", "E516", "E517", "E518", "E519"]

[[row]]
country = "ZM"
data_class = "geoscience"
rule = "aggregates_exportable"
basis = "MRC Act 2024 s.12(2)(c) registered office in Republic for storage of records, s.54; Cyber Security Act 2025 s.8(j), s.9, s.12 (mining CII once designated)"
status = "INFERENCE"
confidence = 0.60
review_by = "2027-01-31"
evidence = ["E520", "E521", "E522", "E523", "E524", "E525"]

[[row]]
country = "ZM"
data_class = "production"
rule = "aggregates_exportable"
basis = "Cyber Security Act 2025 s.12 (information needed to operate CII must be hosted in Zambia once designated); MRC Act 2024 records at registered office"
status = "INFERENCE"
confidence = 0.60
review_by = "2027-01-31"
evidence = ["E520", "E521", "E522", "E523"]

[[row]]
country = "ZM"
data_class = "financial"
rule = "aggregates_exportable"
basis = "Companies Act 2017 (Act 10 of 2017) accounting-records location — not read (403)"
status = "FACT[U]"
confidence = 0.55
review_by = "2027-04-30"
evidence = []

[[row]]
country = "ZM"
data_class = "state_secret"
rule = "in_country_only"
basis = "Cyber Security Act 2025 s.12(1) designated critical information hosted within the Republic; State Security Act (not fetched)"
status = "FACT"
confidence = 0.90
review_by = "2027-04-30"
evidence = ["E520"]

[[row]]
country = "ZM"
data_class = "aggregate"
rule = "exportable"
basis = "Anonymised aggregates are outside DPA 2021; no CSA 2025 designation found"
status = "INFERENCE"
confidence = 0.75
review_by = "2027-01-31"
evidence = ["E522"]

# ---------------- TZ — Tanzania (North Mara, Bulyanhulu) ----------------
[[row]]
country = "TZ"
data_class = "personal"
rule = "in_country_only"
basis = "PDPA 2022 s.31-32; PDP (Collection and Processing) Regs 2023 reg.20-22 (PDPC transfer permit)"
status = "FACT"
confidence = 0.85
review_by = "2027-04-30"
requires = "PDPC permit (Form No. 8) naming recipient and purpose"
evidence = ["E529", "E530"]

[[row]]
country = "TZ"
data_class = "geoscience"
rule = "in_country_only"
basis = "Mining Act Cap 123 R.E. 2023 s.35(2) data owned by Government; s.35(6) no export of core or 'any other data collected' without GST written authorisation; s.125(1)"
status = "FACT"
confidence = 0.90
review_by = "2027-04-30"
requires = "GST written authorisation under s.35(6)"
evidence = ["E526", "E527", "E528"]

[[row]]
country = "TZ"
data_class = "production"
rule = "aggregates_exportable"
basis = "Mining Act s.125(1) records kept in Tanzania; s.35(6) 'any other data collected' scope unclear for production telemetry"
status = "INFERENCE"
confidence = 0.50
review_by = "2027-01-31"
evidence = ["E527", "E528"]

[[row]]
country = "TZ"
data_class = "financial"
rule = "aggregates_exportable"
basis = "Tax Administration Act Cap 438 s.35(1) maintain documents within the United Republic"
status = "FACT"
confidence = 0.75
review_by = "2027-04-30"
evidence = ["E531"]

[[row]]
country = "TZ"
data_class = "state_secret"
rule = "in_country_only"
basis = "National security/secrecy statute (not fetched); class definition"
status = "FACT[U]"
confidence = 0.90
review_by = "2027-04-30"
evidence = []

[[row]]
country = "TZ"
data_class = "aggregate"
rule = "exportable"
basis = "Anonymised KPIs are not personal data; not 'data collected' under s.35(6) (our reading)"
status = "INFERENCE"
confidence = 0.65
review_by = "2027-01-31"
evidence = ["E527"]

# ---------------- CD — DRC (Kibali) ----------------
[[row]]
country = "CD"
data_class = "personal"
rule = "in_country_only"
basis = "Code du numérique (ordonnance-loi 13 Mar 2023) art. 201: stored/hosted in DRC; APD adequacy + prior authorisation (ARPTC interim)"
status = "FACT"
confidence = 0.85
review_by = "2027-04-30"
requires = "APD/ARPTC transfer authorisation"
evidence = ["E532", "E533"]

[[row]]
country = "CD"
data_class = "geoscience"
rule = "aggregates_exportable"
basis = "Code minier art. 50 (samples State property; export visa), art. 216 registers per Règlement minier, art. 324 confidentiality; no digital-data location rule found"
status = "INFERENCE"
confidence = 0.50
review_by = "2027-04-30"
evidence = ["E534", "E535", "E536"]

[[row]]
country = "CD"
data_class = "production"
rule = "aggregates_exportable"
basis = "Code minier art. 216 registers + monthly publication of quantities produced/sold/exported"
status = "INFERENCE"
confidence = 0.50
review_by = "2027-04-30"
evidence = ["E535"]

[[row]]
country = "CD"
data_class = "financial"
rule = "aggregates_exportable"
basis = "OHADA AUDCIF / DRC tax procedure book-location rules — not read"
status = "FACT[U]"
confidence = 0.50
review_by = "2027-04-30"
evidence = []

[[row]]
country = "CD"
data_class = "state_secret"
rule = "in_country_only"
basis = "National secrecy statute (not fetched); class definition"
status = "FACT[U]"
confidence = 0.90
review_by = "2027-04-30"
evidence = []

[[row]]
country = "CD"
data_class = "aggregate"
rule = "exportable"
basis = "Anonymised aggregates outside Code du numérique Livre on personal data; production totals published monthly (art. 216)"
status = "INFERENCE"
confidence = 0.70
review_by = "2027-04-30"
evidence = ["E535"]

# ---------------- ML — Mali (Loulo-Gounkoto) ----------------
[[row]]
country = "ML"
data_class = "personal"
rule = "in_country_only"
basis = "Loi 2013-015 art. 11: transfer only on APDP adequacy finding or APDP decision (contract clauses/internal rules)"
status = "FACT"
confidence = 0.80
review_by = "2027-04-30"
requires = "APDP decision"
evidence = ["E537"]

[[row]]
country = "ML"
data_class = "geoscience"
rule = "aggregates_exportable"
basis = "Code minier 2023 art. 24 (assays in Mali unless written authorisation), art. 194-195; no digital-data export rule found; regime for Loulo-Gounkoto unverified"
status = "INFERENCE"
confidence = 0.50
review_by = "2027-04-30"
evidence = ["E538"]

[[row]]
country = "ML"
data_class = "production"
rule = "aggregates_exportable"
basis = "Code minier 2023 art. 194 site registers; art. 210 sanctions for not keeping extraction/sales/shipping registers"
status = "INFERENCE"
confidence = 0.50
review_by = "2027-04-30"
evidence = ["E538"]

[[row]]
country = "ML"
data_class = "financial"
rule = "aggregates_exportable"
basis = "OHADA AUDCIF / Code général des impôts book-location rules — not read"
status = "FACT[U]"
confidence = 0.50
review_by = "2027-04-30"
evidence = []

[[row]]
country = "ML"
data_class = "state_secret"
rule = "in_country_only"
basis = "National secrecy statute (not fetched); class definition"
status = "FACT[U]"
confidence = 0.90
review_by = "2027-04-30"
evidence = []

[[row]]
country = "ML"
data_class = "aggregate"
rule = "exportable"
basis = "Anonymised aggregates outside Loi 2013-015"
status = "INFERENCE"
confidence = 0.70
review_by = "2027-04-30"
evidence = []

# ---------------- PG — Papua New Guinea (Porgera) ----------------
[[row]]
country = "PG"
data_class = "personal"
rule = "in_country_only"
basis = "No PD statute in force; DGDP Bill 2026 (5th draft) s.81 transfer controls, s.60(4) raw biometrics never offshore"
status = "INFERENCE"
confidence = 0.55
review_by = "2026-12-31"
evidence = ["E540"]

[[row]]
country = "PG"
data_class = "geoscience"
rule = "aggregates_exportable"
basis = "Mining Act 1992 s.161-163 (no location rule); DGDP draft s.82(1)(c) strategic-significance designation; NPL 51% PNG-owned"
status = "INFERENCE"
confidence = 0.55
review_by = "2026-12-31"
evidence = ["E539", "E540"]

[[row]]
country = "PG"
data_class = "production"
rule = "aggregates_exportable"
basis = "DGDP draft s.82(1)(c); no statute in force"
status = "INFERENCE"
confidence = 0.55
review_by = "2026-12-31"
evidence = ["E540"]

[[row]]
country = "PG"
data_class = "financial"
rule = "aggregates_exportable"
basis = "Companies Act 1997 / Income Tax Act record-location rules — not read"
status = "FACT[U]"
confidence = 0.50
review_by = "2027-04-30"
evidence = []

[[row]]
country = "PG"
data_class = "state_secret"
rule = "in_country_only"
basis = "Class definition; DGDP draft s.82(1)(a)-(b)"
status = "FACT[U]"
confidence = 0.90
review_by = "2027-04-30"
evidence = ["E540"]

[[row]]
country = "PG"
data_class = "aggregate"
rule = "exportable"
basis = "Anonymised aggregates; no statute in force"
status = "INFERENCE"
confidence = 0.70
review_by = "2026-12-31"
evidence = []

# ---------------- DO — Dominican Republic (Pueblo Viejo) ----------------
[[row]]
country = "DO"
data_class = "personal"
rule = "in_country_only"
basis = "Ley 172-13: transfer abroad only with consent or listed exceptions (article no. not verified)"
status = "FACT[U]"
confidence = 0.60
review_by = "2027-04-30"
requires = "Data-subject consent or Ley 172-13 exception"
evidence = ["E546"]

[[row]]
country = "DO"
data_class = "geoscience"
rule = "aggregates_exportable"
basis = "Ley Minera 146-71 and Pueblo Viejo Special Lease Agreement not read (R3)"
status = "INFERENCE"
confidence = 0.45
review_by = "2027-04-30"
evidence = []

[[row]]
country = "DO"
data_class = "production"
rule = "aggregates_exportable"
basis = "As geoscience (R3)"
status = "INFERENCE"
confidence = 0.45
review_by = "2027-04-30"
evidence = []

[[row]]
country = "DO"
data_class = "financial"
rule = "aggregates_exportable"
basis = "Código Tributario Ley 11-92 book-keeping duties — location not read"
status = "FACT[U]"
confidence = 0.50
review_by = "2027-04-30"
evidence = []

[[row]]
country = "DO"
data_class = "state_secret"
rule = "in_country_only"
basis = "Class definition"
status = "FACT[U]"
confidence = 0.90
review_by = "2027-04-30"
evidence = []

[[row]]
country = "DO"
data_class = "aggregate"
rule = "exportable"
basis = "Anonymised aggregates outside Ley 172-13"
status = "INFERENCE"
confidence = 0.70
review_by = "2027-04-30"
evidence = []

# ---------------- AR — Argentina (Veladero) ----------------
[[row]]
country = "AR"
data_class = "personal"
rule = "in_country_only"
basis = "Ley 25.326 art. 12; Disp. 60-E/2016 art. 3 as replaced by Res. AAIP 34/2019 (Canada private sector adequate)"
status = "FACT[U]"
confidence = 0.70
review_by = "2027-04-30"
allow_destinations = ["CA"]
requires = "Destination on AAIP adequacy list, or Disp. 60-E/2016 model clauses"
evidence = ["E545"]

[[row]]
country = "AR"
data_class = "geoscience"
rule = "aggregates_exportable"
basis = "Código de Minería / provincial (San Juan) reporting rules not read (R3)"
status = "INFERENCE"
confidence = 0.50
review_by = "2027-04-30"
evidence = []

[[row]]
country = "AR"
data_class = "production"
rule = "aggregates_exportable"
basis = "As geoscience (R3)"
status = "INFERENCE"
confidence = 0.50
review_by = "2027-04-30"
evidence = []

[[row]]
country = "AR"
data_class = "financial"
rule = "aggregates_exportable"
basis = "Código Civil y Comercial arts. 320-331 accounting books — location not read"
status = "FACT[U]"
confidence = 0.55
review_by = "2027-04-30"
evidence = []

[[row]]
country = "AR"
data_class = "state_secret"
rule = "in_country_only"
basis = "Class definition"
status = "FACT[U]"
confidence = 0.90
review_by = "2027-04-30"
evidence = []

[[row]]
country = "AR"
data_class = "aggregate"
rule = "exportable"
basis = "Anonymised aggregates outside Ley 25.326"
status = "INFERENCE"
confidence = 0.75
review_by = "2027-04-30"
evidence = []

# ---------------- CL — Chile (Zaldívar) ----------------
[[row]]
country = "CL"
data_class = "personal"
rule = "in_country_only"
basis = "Ley 19.628 arts. 27-28 as reformed by Ley 21.719 (in force 2026-12-01); model clauses Res. Ex. RAEX202503748 (2025-12-11)"
status = "FACT"
confidence = 0.80
review_by = "2026-12-01"
requires = "Executed model clauses / BCR (art. 27 b) or adequacy (art. 28)"
evidence = ["E542", "E543"]

[[row]]
country = "CL"
data_class = "geoscience"
rule = "exportable"
basis = "DS 9/2024 Min. Minería: submission duty to Sernageomin, no location rule; Ley 21.663 no localisation"
status = "INFERENCE"
confidence = 0.65
review_by = "2027-04-30"
evidence = ["E544"]

[[row]]
country = "CL"
data_class = "production"
rule = "exportable"
basis = "No location rule found (Ley 21.663; Código de Minería not exhaustively read)"
status = "INFERENCE"
confidence = 0.65
review_by = "2027-04-30"
evidence = ["E544"]

[[row]]
country = "CL"
data_class = "financial"
rule = "exportable"
basis = "Código Tributario art. 17: Spanish, conservation, SII-approved systems; no location rule"
status = "INFERENCE"
confidence = 0.60
review_by = "2027-04-30"
evidence = ["E544"]

[[row]]
country = "CL"
data_class = "state_secret"
rule = "in_country_only"
basis = "Class definition"
status = "FACT[U]"
confidence = 0.90
review_by = "2027-04-30"
evidence = []

[[row]]
country = "CL"
data_class = "aggregate"
rule = "exportable"
basis = "Anonymised data outside Ley 19.628"
status = "INFERENCE"
confidence = 0.80
review_by = "2027-04-30"
evidence = []

# ---------------- SA — Saudi Arabia (Jabal Sayid) ----------------
[[row]]
country = "SA"
data_class = "personal"
rule = "in_country_only"
basis = "PDPL art. 29 + Transfer Regulation (amended Sep 2024): SCCs/BCRs/certification"
status = "FACT[U]"
confidence = 0.75
review_by = "2027-04-30"
requires = "SDAIA SCCs (unmodified) or BCR; risk assessment for sensitive/continuous transfers"
evidence = ["E547"]

[[row]]
country = "SA"
data_class = "geoscience"
rule = "aggregates_exportable"
basis = "Mining Investment Law not read; NDMO localisation may apply if JV is a government-affiliated company (ECC-2:2024 scope)"
status = "INFERENCE"
confidence = 0.50
review_by = "2027-04-30"
evidence = ["E547"]

[[row]]
country = "SA"
data_class = "production"
rule = "aggregates_exportable"
basis = "As geoscience (R3)"
status = "INFERENCE"
confidence = 0.50
review_by = "2027-04-30"
evidence = ["E547"]

[[row]]
country = "SA"
data_class = "financial"
rule = "aggregates_exportable"
basis = "Saudi Companies Law / ZATCA record rules — not read"
status = "FACT[U]"
confidence = 0.50
review_by = "2027-04-30"
evidence = []

[[row]]
country = "SA"
data_class = "state_secret"
rule = "in_country_only"
basis = "Class definition; NDMO data-classification controls"
status = "FACT[U]"
confidence = 0.90
review_by = "2027-04-30"
evidence = ["E547"]

[[row]]
country = "SA"
data_class = "aggregate"
rule = "exportable"
basis = "Anonymised aggregates outside PDPL"
status = "INFERENCE"
confidence = 0.65
review_by = "2027-04-30"
evidence = []

# ---------------- US — USA (Nevada) ----------------
[[row]]
country = "US"
data_class = "personal"
rule = "exportable"
basis = "No general localisation; 28 CFR Part 202 bars bulk sensitive personal data to countries of concern"
status = "FACT[U]"
confidence = 0.75
review_by = "2027-04-30"
deny_destinations = ["CN", "HK", "MO", "RU", "IR", "KP", "CU", "VE"]
evidence = ["E548"]

[[row]]
country = "US"
data_class = "geoscience"
rule = "exportable"
basis = "No federal/Nevada data-location rule identified"
status = "INFERENCE"
confidence = 0.75
review_by = "2027-04-30"
evidence = []

[[row]]
country = "US"
data_class = "production"
rule = "exportable"
basis = "No federal/Nevada data-location rule identified"
status = "INFERENCE"
confidence = 0.75
review_by = "2027-04-30"
evidence = []

[[row]]
country = "US"
data_class = "financial"
rule = "exportable"
basis = "IRS requires availability, not location (not re-verified)"
status = "INFERENCE"
confidence = 0.70
review_by = "2027-04-30"
evidence = []

[[row]]
country = "US"
data_class = "state_secret"
rule = "in_country_only"
basis = "Classified national security information (EO 13526) — class definition"
status = "FACT[U]"
confidence = 0.95
review_by = "2027-04-30"
evidence = []

[[row]]
country = "US"
data_class = "aggregate"
rule = "exportable"
basis = "Anonymised aggregates"
status = "INFERENCE"
confidence = 0.85
review_by = "2027-04-30"
evidence = []

# ---------------- CA — Canada (head office) ----------------
[[row]]
country = "CA"
data_class = "personal"
rule = "exportable"
basis = "PIPEDA: no localisation (accountability for transfers); Quebec private-sector Act s.17 PIA for Quebec PI (not fetched)"
status = "FACT[U]"
confidence = 0.75
review_by = "2027-04-30"
evidence = []

[[row]]
country = "CA"
data_class = "geoscience"
rule = "exportable"
basis = "No federal/Ontario data-location rule identified"
status = "INFERENCE"
confidence = 0.75
review_by = "2027-04-30"
evidence = []

[[row]]
country = "CA"
data_class = "production"
rule = "exportable"
basis = "No federal/Ontario data-location rule identified"
status = "INFERENCE"
confidence = 0.75
review_by = "2027-04-30"
evidence = []

[[row]]
country = "CA"
data_class = "financial"
rule = "aggregates_exportable"
basis = "Income Tax Act s.230(1): records at place of business or residence in Canada or Minister-designated place"
status = "FACT"
confidence = 0.80
review_by = "2027-04-30"
evidence = ["E549"]

[[row]]
country = "CA"
data_class = "state_secret"
rule = "in_country_only"
basis = "Security of Information Act — class definition (not fetched)"
status = "FACT[U]"
confidence = 0.90
review_by = "2027-04-30"
evidence = []

[[row]]
country = "CA"
data_class = "aggregate"
rule = "exportable"
basis = "Anonymised aggregates"
status = "INFERENCE"
confidence = 0.85
review_by = "2027-04-30"
evidence = []
```

**Row totals (72):**

| | Count |
|---|---|
| `in_country_only` | 23 |
| `aggregates_exportable` | 27 |
| `exportable` | 22 |
| FACT | 10 |
| FACT[U] | 23 |
| INFERENCE | 39 |

## 5. Paste-ready plan text

**§26.14, replace the OPEN-B4-01 bullet with:**

```markdown
- **OPEN-B4-01 (narrowed 2026-10-29).** Statutes are now cited row by row in `registries/residency.toml`
  (W3_data_residency.md, E501–E549). Hard localisation in force: ZM personal (DPA 2021 s.70), TZ geoscience
  export (Mining Act s.35(6)) and personal (PDP Regs 2023 reg.20 permit), CD personal (Code du numérique art.201),
  ML personal (Loi 2013-015 art.11). Pakistan has no data-protection statute in force; Reko Diq is governed by its
  mineral agreement and BMR 2002 (BMMA 2025 s.122(4)): records in Balochistan (r.56), State data rights (r.71).
  Counsel is still needed on eight named clauses (W3 §7). Unknown pairs still default to `in_country_only`.
```

**§31.2, replace the "The statutes themselves are OPEN-B4-01" bullet with:**

```markdown
- Each row in `registries/residency.toml` cites its statute, status (FACT, FACT[U], INFERENCE) and `review_by`
  (OPEN-B4-01, narrowed; W3). An unknown `(country, class)` pair still defaults to `in_country_only`.
  Pakistan (Reko Diq): `personal` and `state_secret` are `in_country_only`; `geoscience`, `production` and
  `financial` are `aggregates_exportable` (BMR 2002 r.56, r.71; Companies Act 2017 s.220); `aggregate` is
  `exportable`. Zambia (M2): `personal` is `in_country_only` (DPA 2021 s.70), so Muster needs an in-country
  regional node from day one.
```

## 6. Evidence rows E501–E549

All rows accessed 2026-10-29. "pk/…" and similar paths are relative to the scratch `w3/` folder (§2).

| ID | Class | Claim | Source + locator | Access |
|---|---|---|---|---|
| E501 | FACT | BMMA 2025 is Act XIII of 2025: passed 12 Mar, assented 14 Mar, notified 17 Mar 2025; "shall come into operation at once" (s.2) | S01 p.1 (OCR `pk/ocrtxt/all.txt` l.1–40) | 2026-10-29 |
| E502 | FACT | BMMA s.3(2) disapplies the Act to mineral agreements and Qualified Investments under s.122(4); s.4 "Qualified Investment" "specifically includes the Reko Diq project" | S01 p.1, p.9 (OCR l.40–46, l.650–657) | 2026-10-29 |
| E503 | FACT | BMMA s.122(4): such agreements "remain unaffected by this Act in all respects", governed by their terms, enabling legislation and BMR 2002 | S01 p.109–110 (OCR l.7078–7111) | 2026-10-29 |
| E504 | FACT | BMR 2002 r.56(1): lessee keeps "at an address in Balochistan" records of production, geology, assays, metallurgy, capex/opex | S02 r.56 (`pk/bmr2002.txt` l.2775–2826) | 2026-10-29 |
| E505 | FACT | BMR r.71(1): Government has "exclusive right to all data including geological ... cores and production data ... interpretative and derivative data" | S02 r.71 (l.3342–3347) | 2026-10-29 |
| E506 | FACT | BMR r.71(2)–(3): holder may use and retain copies; non-public data not disclosed without consent except "in connection with, exploration and mining operations" or "as required by law" | S02 r.71 (l.3349–3364) | 2026-10-29 |
| E507 | FACT | BMR r.5: information disclosed to the licensing authority is confidential, with listed exceptions | S02 r.5 (l.555–577) | 2026-10-29 |
| E508 | FACT | Companies Act 2017 s.220(1): books at registered office or "such other place in Pakistan as the board may decide"; s.220(3): copies of financial info "maintained outside the country" | S03 s.220 (`pk/ca2017.txt` l.6534–6565) | 2026-10-29 |
| E509 | FACT | PECA 2016 s.2(j): Government may designate private infrastructure as critical; s.7 criminalises unauthorised copying or transmission of critical infrastructure data | S04 (`pk/peca.txt` l.76–91, l.235–238) | 2026-10-29 |
| E510 | FACT | Cloud First Policy applies to federal public-sector entities and is "guidance" for private ones; Government and private cloud "can only be located in Pakistan"; Annex E maps Restricted, Sensitive and Secret data to those clouds | S05 §5, §7.2–7.3, Annex D–E (`pk/cloudfirst.txt` l.286–290, 359–370, 839–914) | 2026-10-29 |
| E511 | FACT[secondary] | PDP Bill (2023 draft) s.31(2): "Critical Personal Data shall only be processed in a server(s) or digital infrastructure located within the territory of Pakistan"; s.32 transfer framework | S06 pp.2–3 (`pk/drf.txt` l.44–70, 88–107) | 2026-10-29 |
| E512 | FACT[secondary] | PDP Bill has "yet to be passed by both houses"; Pakistan has no operative data protection law (guide updated 10 Mar 2026); no enactment found by 2026-10-29 | S07; web search 2026-10-29 | 2026-10-29 |
| E513 | INFERENCE | Reko Diq's data obligations ultimately sit in the non-public mineral agreement and definitive agreements; RDMC is assumed Pakistan-incorporated (not verified) | Derived from E502–E503 | 2026-10-29 |
| E514 | FACT | ZM DPA s.70(1): "A data controller shall process and store personal data on a server or data centre located in the Republic." | S08 s.70 (`zm/dpa2021.txt` l.1427–1428) | 2026-10-29 |
| E515 | FACT | ZM DPA s.70(2): Minister may prescribe categories storable abroad; s.70(3): sensitive personal data "shall be processed and stored" in Zambia | S08 s.70 (l.1431–1435) | 2026-10-29 |
| E516 | FACT | ZM DPA s.71(1): transfer needs consent plus ODPC-approved standard contracts or intragroup schemes, or ministerial prescription, or ODPC necessity approval; s.71(4)(b) explicit consent for sensitive data | S08 s.71 (l.1438–1478) | 2026-10-29 |
| E517 | FACT | ZM "sensitive personal data" includes sex, marital status, biometric, genetic and health data | S08 s.2 (l.309–322) | 2026-10-29 |
| E518 | FACT | ZM DPA brought into operation by SI 22 of 2021 (dated 31 Mar 2021, gazetted 1 Apr 2021) | S09 (`zm/si22.txt`) | 2026-10-29 |
| E519 | INFERENCE | No s.70(2) statutory instrument and no ODPC-approved intragroup scheme was located | Web search 2026-10-29 (afriwise, DLA Piper summaries) | 2026-10-29 |
| E520 | FACT | ZM CSA 2025 s.8(j): mining is a critical sector; s.9 Gazette designation; s.12(1): "A controller shall host critical information or critical information infrastructure within the Republic"; s.12(2): Agency may authorise offshore hosting | S10 ss.8–12 (`zm/csa2025.txt` l.760–874) | 2026-10-29 |
| E521 | FACT | ZM CSA "controller" means a person responsible for CI/CII "that is registered under this Act"; "critical information" includes personal data processed by a controller and information needed to operate CII | S10 s.2 (l.337–353) | 2026-10-29 |
| E522 | FACT[secondary] | ZM CSA 2025 commenced 12 May 2025; no Gazette designation of mining information found | S10 front matter (l.235–237); web search 2026-10-29 | 2026-10-29 |
| E523 | FACT | ZM MRC Act s.12(2)(c): no mining right for a company without "a registered office in the Republic"; "registered office" includes "storage of records"; s.54: records surrendered on termination | S12 s.2, s.12, s.54 (`zm/mrca2024.txt` l.591–592, 882–895, 2125–2134) | 2026-10-29 |
| E524 | FACT[secondary] | ZM MRC Act commenced 3 Jun 2025 (SI 42 of 2025); it repeals the MMDA 2015 | S14; S12 long title (l.299–302) | 2026-10-29 |
| E525 | FACT | ZM GMDA 2025 s.11: restriction on removal of minerals and samples; s.17: officers' non-disclosure; no digital-data export rule | S13 ss.11, 17 (`zm/gmda2025_b.txt` l.437–450, 538–554) | 2026-10-29 |
| E526 | FACT | TZ Mining Act s.35(2): "All mineral data generated under this Act shall be owned by the Government"; s.35(3): submission list to GST | S17 s.35 (`tz/mining_act.txt` l.3156–3199) | 2026-10-29 |
| E527 | FACT | TZ Mining Act s.35(6): "shall not export any core, cuttings, rock samples, soil, fluid samples or any other data collected without the written authorisation of the Geological Survey of Tanzania" | S17 s.35(6) (l.3212–3218) | 2026-10-29 |
| E528 | FACT | TZ Mining Act s.125(1): records kept "within the mining areas or at the mineral rights holders' office located in Tanzania" | S17 s.125 (l.8219–8231) | 2026-10-29 |
| E529 | FACT | TZ PDPA 2022 s.31(1): Commission "may ... prohibit the transfer of personal data to a place outside the country"; s.32 adequacy and derogations | S15 ss.31–32 (`tz/pdpa2022.txt` l.906–990) | 2026-10-29 |
| E530 | FACT | TZ Regs 2023 reg.20(1): permit application (Form No. 7) to transfer outside the country; reg.21(c): refusal if "restricted by other written laws"; reg.22: permit conditions | S16 regs 20–22 (`tz/regs2023.txt` l.591–682) | 2026-10-29 |
| E531 | FACT | TZ Tax Administration Act s.35(1): "shall, within the United Republic, maintain documents in paper or electronic form" | S18 s.35 (`tz/taa438.txt` l.1439–1449) | 2026-10-29 |
| E532 | FACT | CD Code du numérique art. 201: data "stockées et/ou hébergées en République Démocratique du Congo"; transfer needs APD adequacy finding and prior APD authorisation | S19 art. 201 (`cd/code_numerique.txt` l.3777–3820) | 2026-10-29 |
| E533 | FACT[secondary] | CD: APD not yet established; ARPTC exercises its powers provisionally; ARPTC decision of 11 Sep 2025 sets procedures including transfer conditions (legality contested) | S22 (`cd/illegalite.txt` l.16–31, 129–133, 198–202) | 2026-10-29 |
| E534 | FACT | CD Code minier art. 50 (2018): samples are "propriété de l'Etat"; sending samples abroad requires the Direction de géologie visa | S20 (`cd/loi18001.txt` l.2913–2924) | 2026-10-29 |
| E535 | FACT | CD art. 216 (2018): registers and reports per Règlement minier; monthly publication of quantities produced, sold and exported | S20 (l.1762–1772) | 2026-10-29 |
| E536 | FACT | CD art. 324 (2002 text): technical, geological and mining information supplied is confidential for ten years | S21 (`cd/code_minier_2002.txt` l.6633–6639) | 2026-10-29 |
| E537 | FACT | ML Loi 2013-015 art. 11: transfer only on APDP adequacy finding or APDP decision based on contractual clauses or internal rules | S23 art. 11 (`ml/loi2013015.txt` l.274–288) | 2026-10-29 |
| E538 | FACT | ML Code minier 2023 art. 24: sample analyses in Mali, abroad only with written authorisation; art. 194 site registers; art. 195 confidentiality | S24 (`ml/cmin2023_flow.txt` l.745–751, 3026–3059) | 2026-10-29 |
| E539 | FACT | PG Mining Act 1992 ss.161–163: information duty, core preservation and confidentiality; no data-location rule | S25 (`pg/mining_act_1992.txt` l.3486–3535) | 2026-10-29 |
| E540 | FACT | PG DGDP Act 2026 5th draft: s.81(1) transfer conditions; s.60(4) raw biometrics never offshore; s.82(1)(c) strategic-significance hosting; consultation closed 19 Aug 2026 (bill, not law) | S29 (`pg/dgdp2026_draft5.txt` l.1201–1216, 1470–1504, 1515–1522); S28 l.253 | 2026-10-29 |
| E541 | FACT | PG Government Cloud Policy 2023: "Public bodies ... with the exception of state-owned entities" | S27 Definitions (`pg/cloudpolicy.txt` l.91–93) | 2026-10-29 |
| E542 | FACT | CL Ley 21.719 published 2024-12-13; in force on "el día primero del mes vigésimo cuarto posterior", i.e. 2026-12-01; new arts. 27–28 on international transfers | S30 transitory art. 1 (`cl/l21719.txt` l.1464–1468; xml `fechaPublicacion`); S31 art. 27 (l.2023–2056) | 2026-10-29 |
| E543 | FACT | CL Res. Ex. RAEX202503748 (11 Dec 2025) approves model contractual clauses for international transfers | S32 (`cl/cct.txt` l.8–18) | 2026-10-29 |
| E544 | FACT | CL DS 9/2024 regulates delivery of geological information to Sernageomin (submission duty); Código Tributario art. 17 has no book-location rule | S33 arts. 1–6; S35 art. 17 (`cl/dl830.txt` l.1329–1371) | 2026-10-29 |
| E545 | FACT[secondary] | AR Ley 25.326 art. 12 bans transfer to non-adequate countries; Disp. 60-E/2016 art. 3 (replaced by Res. AAIP 34/2019) lists Canada (private sector) as adequate | S36 (search summary of argentina.gob.ar and Boletín Oficial) | 2026-10-29 |
| E546 | FACT[secondary] | DO Ley 172-13 (13 Dec 2013): transfers abroad only with consent or listed exceptions | S37 | 2026-10-29 |
| E547 | FACT | SA NCA ECC-2:2024 scope covers government agencies' "affiliated companies and entities" and private CNI operators; sub-control 4-2-3-3 (hosting inside KSA) deleted and localisation moved to NDMO | S38 Scope (`sa/ecc2_2024.txt` l.308–316); change table (l.2017–2031). PDPL art. 29 and Transfer Regulation are secondary (S39) | 2026-10-29 |
| E548 | FACT[secondary] | US 28 CFR Part 202 effective 2025-04-08 restricts bulk US sensitive personal data to countries of concern; no general localisation | S40 | 2026-10-29 |
| E549 | FACT | CA ITA s.230(1): records "at the person's place of business or residence in Canada or at such other place as may be designated by the Minister" (current to 2026-09-30) | S41 | 2026-10-29 |

## 7. Questions that genuinely need counsel

Ranked by impact on the build.

1. **PK: Reko Diq mineral and definitive agreements, and the reading of BMR r.71(3)(i).**
   - BMMA s.122(4) makes the non-public mineral agreement and BMR 2002 the governing texts. Does the agreement contain data-ownership, confidentiality or record-location clauses that override r.56 and r.71?
   - Does r.71(3)(i), "as may be necessary for the purpose of, or in connection with, exploration and mining operations", permit replication of raw geoscience and production data to Barrick group systems or a cloud provider outside Pakistan? Or is each recipient a "person" needing the licensing authority's prior consent?
   - Does FIPPA 2022 stabilisation shield RDMC from a future PDP Act's localisation of "critical personal data"?
   - Confirm that RDMC is Pakistan-incorporated, so that Companies Act 2017 s.220 applies.
   - Confirm that the 2008 BMR edition is current, i.e. any later amendments (2022+).
2. **ZM: DPA s.70(3) versus s.71(4)(b), and CSA 2025 designation.**
   - s.70(3) says sensitive data "shall be processed and stored" in Zambia "despite" s.70(2). s.71(4)(b) allows transfer with explicit consent. Is consent enough to export, or does it permit only a copy while primary processing stays in Zambia? Because "sex" is sensitive, this decides the fate of every HR, roster and fatigue record at Lumwana.
   - Has the ODPC approved any intragroup scheme?
   - Is the Zambia Cyber Security Agency preparing a s.9 designation for mining? If Lumwana systems are designated, s.12 forces `production` (and possibly `geoscience`) to `in_country_only`.
3. **TZ: scope of Mining Act s.35(6), "or any other data collected".**
   - Read with s.35(2) ("all mineral data"), does it reach production, metallurgical and plant telemetry, or only exploration and geological data submitted to the GST?
   - Does an existing GST authorisation cover Barrick group geoscience workflows for North Mara and Bulyanhulu?
4. **CD: validity of the transfer regime.**
   - Has the Code du numérique ordonnance-loi been ratified by Parliament?
   - Are ARPTC's interim transfer authorisations (decision of 11 Sep 2025) valid, given the challenge in S22?
   - Where does the Règlement minier (as amended in 2018) require Kibali's registers to be kept?
5. **SA: is Ma'aden Barrick Copper Company an "affiliated company" of a government agency** (through Ma'aden and PIF) within ECC-2:2024 and NDMO scope? If yes, NDMO localisation controls apply to every class at Jabal Sayid.
6. **ML: which regime governs Loulo-Gounkoto after the 2025 dispute and settlement** — the 2023 Code (art. 24, assays in Mali) or the earlier convention? Does the settlement include data-access or data-location terms?
7. **PG: designation and status risk.**
   - If the DGDP Bill passes, is Porgera data likely to be designated "of strategic significance" (draft s.82(1)(c))?
   - Does New Porgera's 51% state and landowner ownership bring it within "government agency" duties under draft s.83(1)?
   - Would the s.82(2) "sovereign hosting arrangement" test accept a US-provider region, given the CLOUD Act?
8. **CL and AR transfer instruments.**
   - CL: confirm that Ley 21.719 model clauses (or BCRs) are executed intra-group before 2026-12-01.
   - AR: confirm whether Barrick's enterprise node will sit in Canada, which is AAIP-adequate for the private sector. Otherwise Disp. 60-E/2016 model clauses are needed.

**Gaps left as FACT[U] (counsel or a follow-up pass):**
- State-secrets statutes for all 12 countries.
- Company and tax book-location rules for ZM, CD, ML, PG, DO, AR and SA.
- Mining-data rules for DO (Pueblo Viejo SLA), AR (San Juan) and SA (Mining Investment Law).
- The primary texts of Ley 172-13 (DO) and PDPL art. 29 (SA).
