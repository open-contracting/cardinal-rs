# CSV data description

Bulk tender CSV uses semicolon (;) delimiters, UTF-8 encoding, and one row per combination of buyer × lot × bid × bidder. 

Dates are YYYY/MM/DD. Boolean fields use yes/no. Price groups expand to national net amount, currency, optional min/max range, and EUR. 

Tender rows repeat for each nested combination; empty nested entities yield blank cells in those columns. 

Indicator columns follow the schema enum in opentender-data. 

## Tender

| Column | Description |
| :--- | :--- |
| `tender_row_nr` | Row index for this tender within the export batch (1-based). |
| `tender_id` | Stable tender identifier in OpenTender. |
| `tender_country` | ISO-style country code for the portal source. |
| `tender_title` | Tender title as published. |
| `tender_size` | Estimated contract size band or classification when available. |
| `tender_supplyType` | Supply type (works, services, supplies, etc.) when coded. |
| `tender_procedureType` | Procurement procedure type (open, restricted, negotiated, etc.). |
| `tender_nationalProcedureType` | National procedure classification when provided. |
| `tender_mainCpv` | Main CPV code (single code marked as main in source data). |
| `tender_cpvs` | Comma-separated list of all CPV codes on the tender. |
| `tender_addressOfImplementation_nuts` | Comma-separated NUTS region codes for place of performance. |
| `tender_addressOfImplementation_country` | Country for place of performance. |
| `tender_addressOfImplementation_postcode` | Postcode for place of performance. |
| `tender_addressOfImplementation_city` | City for place of performance (column name matches CSV header, including trailing space). |
| `tender_addressOfImplementation_street` | Street address for place of performance (column name matches CSV header, including trailing space). |
| `tender_year` | Four-digit year derived from OpenTender processing date (\`ot.date\`). |
| `tender_fundingProgrammes` | Comma-separated EU or other funding programme names linked to the tender. |
| `tender_eligibleBidLanguages` | Comma-separated languages in which bids may be submitted. |
| `tender_npwp_reasons` | Comma-separated non-price weighting / award-criteria reasons when present. |
| `tender_awardDeadline` | Award decision deadline (YYYY/MM/DD). |
| `tender_contractSignatureDate` | Contract signature date (YYYY/MM/DD). |
| `tender_awardDecisionDate` | Award decision date (YYYY/MM/DD). |
| `tender_bidDeadline` | Bid submission deadline (YYYY/MM/DD). |
| `tender_cancellationDate` | Cancellation date if the procedure was cancelled (YYYY/MM/DD). |
| `tender_estimatedStartDate` | Estimated contract start (YYYY/MM/DD). |
| `tender_estimatedCompletionDate` | Estimated contract end (YYYY/MM/DD). |
| `tender_estimatedDurationInYears` | Estimated duration in years (numeric). |
| `tender_estimatedDurationInMonths` | Estimated duration in months (numeric). |
| `tender_estimatedDurationInDays` | Estimated duration in days (numeric). |
| `tender_isEUFunded` | Whether EU funding is linked (\`yes\` / \`no\`). |
| `tender_isDps` | Dynamic purchasing system flag (\`yes\` / \`no\`). |
| `tender_isElectronicAuction` | Electronic auction used (\`yes\` / \`no\`). |
| `tender_isAwarded` | Tender has at least one award (\`yes\` / \`no\`). |
| `tender_isCentralProcurement` | Central purchasing body (\`yes\` / \`no\`). |
| `tender_isJointProcurement` | Joint procurement between buyers (\`yes\` / \`no\`). |
| `tender_isOnBehalfOf` | Procurement on behalf of another authority (\`yes\` / \`no\`). |
| `tender_isFrameworkAgreement` | Framework agreement (\`yes\` / \`no\`). |
| `tender_isCoveredByGpa` | Covered by GPA (\`yes\` / \`no\`). |
| `tender_hasLots` | Tender divided into lots (\`yes\` / \`no\`). |
| `tender_estimatedPrice` | Estimated value: net amount in national currency. |
| `tender_estimatedPrice_currency` | Estimated value: national currency code. |
| `tender_estimatedPrice_minNetAmount` | Estimated value: minimum net amount when a range is given. |
| `tender_estimatedPrice_maxNetAmount` | Estimated value: maximum net amount when a range is given. |
| `tender_estimatedPrice_EUR` | Estimated value: net amount converted to EUR when available. |
| `tender_finalPrice` | Final / awarded value: net amount in national currency. |
| `tender_finalPrice_currency` | Final value: national currency code. |
| `tender_finalPrice_minNetAmount` | Final value: minimum net amount when a range is given. |
| `tender_finalPrice_maxNetAmount` | Final value: maximum net amount when a range is given. |
| `tender_finalPrice_EUR` | Final value: net amount in EUR when available. |
| `tender_description` | Full tender description text. |
| `tender_description_length` | Character length of the tender description. |
| `tender_personalRequirements_length` | Character length of personal suitability requirements text. |
| `tender_economicRequirements_length` | Character length of economic / financial requirements text. |
| `tender_technicalRequirements_length` | Character length of technical requirements text. |
| `tender_documents_count` | Number of document objects attached to the tender. |
| `tender_awardCriteria_count` | Number of award criteria entries. |
| `tender_corrections_count` | Number of correction notices linked to the tender. |
| `tender_onBehalfOf_count` | Number of on-behalf-of relationships. |
| `tender_lots_count` | Number of lots. |
| `tender_publications_count` | Number of publication entries. |
| `tender_publications_firstCallForTenderDate` | Earliest contract-notice publication date (YYYY/MM/DD). |
| `tender_publications_lastCallForTenderDate` | Latest contract-notice publication date (YYYY/MM/DD). |
| `tender_publications_firstdContractAwardDate` | Earliest contract-award publication date (YYYY/MM/DD). Note the typo \`firstd\` in the column name matches the export. |
| `tender_publications_lastContractAwardDate` | Latest contract-award publication date (YYYY/MM/DD). |
| `tender_publications_lastContractAwardUrl` | Human-readable URL of the latest contract-award notice when available. |
| `tender_buyerAssignedId` | Buyer-assigned reference ID (column name matches CSV header, including trailing space). |
| `tender_indicator_INTEGRITY_SINGLE_BID` | Tender-level indicator score for \`INTEGRITY\_SINGLE\_BID\` (0–100 when computed, empty otherwise). Integrity, transparency, and administrative indicators are defined in the OpenTender schema. |
| `tender_indicator_INTEGRITY_CALL_FOR_TENDER_PUBLICATION` | Tender-level indicator score for \`INTEGRITY\_CALL\_FOR\_TENDER\_PUBLICATION\` (0–100 when computed, empty otherwise). Integrity, transparency, and administrative indicators are defined in the OpenTender schema. |
| `tender_indicator_INTEGRITY_ADVERTISEMENT_PERIOD` | Tender-level indicator score for \`INTEGRITY\_ADVERTISEMENT\_PERIOD\` (0–100 when computed, empty otherwise). Integrity, transparency, and administrative indicators are defined in the OpenTender schema. |
| `tender_indicator_INTEGRITY_PROCEDURE_TYPE` | Tender-level indicator score for \`INTEGRITY\_PROCEDURE\_TYPE\` (0–100 when computed, empty otherwise). Integrity, transparency, and administrative indicators are defined in the OpenTender schema. |
| `tender_indicator_INTEGRITY_DECISION_PERIOD` | Tender-level indicator score for \`INTEGRITY\_DECISION\_PERIOD\` (0–100 when computed, empty otherwise). Integrity, transparency, and administrative indicators are defined in the OpenTender schema. |
| `tender_indicator_INTEGRITY_TAX_HAVEN` | Tender-level indicator score for \`INTEGRITY\_TAX\_HAVEN\` (0–100 when computed, empty otherwise). Integrity, transparency, and administrative indicators are defined in the OpenTender schema. |
| `tender_indicator_INTEGRITY_NEW_COMPANY` | Tender-level indicator score for \`INTEGRITY\_NEW\_COMPANY\` (0–100 when computed, empty otherwise). Integrity, transparency, and administrative indicators are defined in the OpenTender schema. |
| `tender_indicator_INTEGRITY_DESCRIPTION_LENGTH` | Tender-level indicator score for \`INTEGRITY\_DESCRIPTION\_LENGTH\` (0–100 when computed, empty otherwise). Integrity, transparency, and administrative indicators are defined in the OpenTender schema. |
| `tender_indicator_INTEGRITY_SIGNATURE_PERIOD` | Tender-level indicator score for \`INTEGRITY\_SIGNATURE\_PERIOD\` (0–100 when computed, empty otherwise). Integrity, transparency, and administrative indicators are defined in the OpenTender schema. |
| `tender_indicator_INTEGRITY_WINNER_CONTRACT_SHARE` | Tender-level indicator score for \`INTEGRITY\_WINNER\_CONTRACT\_SHARE\` (0–100 when computed, empty otherwise). Integrity, transparency, and administrative indicators are defined in the OpenTender schema. |
| `tender_indicator_INTEGRITY_POLITICAL_CONNECTIONS` | Tender-level indicator score for \`INTEGRITY\_POLITICAL\_CONNECTIONS\` (0–100 when computed, empty otherwise). Integrity, transparency, and administrative indicators are defined in the OpenTender schema. |
| `tender_indicator_INTEGRITY_BUYER_CONCENTRATION` | Tender-level indicator score for \`INTEGRITY\_BUYER\_CONCENTRATION\` (0–100 when computed, empty otherwise). Integrity, transparency, and administrative indicators are defined in the OpenTender schema. |
| `tender_indicator_INTEGRITY_SUPPLIER_SHARE_IN_BUYER_SPENDING` | Tender-level indicator score for \`INTEGRITY\_SUPPLIER\_SHARE\_IN\_BUYER\_SPENDING\` (0–100 when computed, empty otherwise). Integrity, transparency, and administrative indicators are defined in the OpenTender schema. |
| `tender_indicator_INTEGRITY_BENFORDS_LAW_FOR_BID_PRICES` | Tender-level indicator score for \`INTEGRITY\_BENFORDS\_LAW\_FOR\_BID\_PRICES\` (0–100 when computed, empty otherwise). Integrity, transparency, and administrative indicators are defined in the OpenTender schema. |
| `tender_indicator_INTEGRITY_DISTINCT_MARKET` | Tender-level indicator score for \`INTEGRITY\_DISTINCT\_MARKET\` (0–100 when computed, empty otherwise). Integrity, transparency, and administrative indicators are defined in the OpenTender schema. |
| `tender_indicator_ADMINISTRATIVE_CENTRALIZED_PROCUREMENT` | Tender-level indicator score for \`ADMINISTRATIVE\_CENTRALIZED\_PROCUREMENT\` (0–100 when computed, empty otherwise). Integrity, transparency, and administrative indicators are defined in the OpenTender schema. |
| `tender_indicator_ADMINISTRATIVE_ELECTRONIC_AUCTION` | Tender-level indicator score for \`ADMINISTRATIVE\_ELECTRONIC\_AUCTION\` (0–100 when computed, empty otherwise). Integrity, transparency, and administrative indicators are defined in the OpenTender schema. |
| `tender_indicator_ADMINISTRATIVE_COVERED_BY_GPA` | Tender-level indicator score for \`ADMINISTRATIVE\_COVERED\_BY\_GPA\` (0–100 when computed, empty otherwise). Integrity, transparency, and administrative indicators are defined in the OpenTender schema. |
| `tender_indicator_ADMINISTRATIVE_FRAMEWORK_AGREEMENT` | Tender-level indicator score for \`ADMINISTRATIVE\_FRAMEWORK\_AGREEMENT\` (0–100 when computed, empty otherwise). Integrity, transparency, and administrative indicators are defined in the OpenTender schema. |
| `tender_indicator_ADMINISTRATIVE_ENGLISH_AS_FOREIGN_LANGUAGE` | Tender-level indicator score for \`ADMINISTRATIVE\_ENGLISH\_AS\_FOREIGN\_LANGUAGE\` (0–100 when computed, empty otherwise). Integrity, transparency, and administrative indicators are defined in the OpenTender schema. |
| `tender_indicator_ADMINISTRATIVE_NOTICE_AND_AWARD_DISCREPANCIES` | Tender-level indicator score for \`ADMINISTRATIVE\_NOTICE\_AND\_AWARD\_DISCREPANCIES\` (0–100 when computed, empty otherwise). Integrity, transparency, and administrative indicators are defined in the OpenTender schema. |
| `tender_indicator_TRANSPARENCY_NUMBER_OF_KEY_MISSING_FIELDS` | Tender-level indicator score for \`TRANSPARENCY\_NUMBER\_OF\_KEY\_MISSING\_FIELDS\` (0–100 when computed, empty otherwise). Integrity, transparency, and administrative indicators are defined in the OpenTender schema. |
| `tender_indicator_TRANSPARENCY_AWARD_DATE_MISSING` | Tender-level indicator score for \`TRANSPARENCY\_AWARD\_DATE\_MISSING\` (0–100 when computed, empty otherwise). Integrity, transparency, and administrative indicators are defined in the OpenTender schema. |
| `tender_indicator_TRANSPARENCY_BUYER_NAME_MISSING` | Tender-level indicator score for \`TRANSPARENCY\_BUYER\_NAME\_MISSING\` (0–100 when computed, empty otherwise). Integrity, transparency, and administrative indicators are defined in the OpenTender schema. |
| `tender_indicator_TRANSPARENCY_PROC_METHOD_MISSING` | Tender-level indicator score for \`TRANSPARENCY\_PROC\_METHOD\_MISSING\` (0–100 when computed, empty otherwise). Integrity, transparency, and administrative indicators are defined in the OpenTender schema. |
| `tender_indicator_TRANSPARENCY_BUYER_LOC_MISSING` | Tender-level indicator score for \`TRANSPARENCY\_BUYER\_LOC\_MISSING\` (0–100 when computed, empty otherwise). Integrity, transparency, and administrative indicators are defined in the OpenTender schema. |
| `tender_indicator_TRANSPARENCY_BIDDER_ID_MISSING` | Tender-level indicator score for \`TRANSPARENCY\_BIDDER\_ID\_MISSING\` (0–100 when computed, empty otherwise). Integrity, transparency, and administrative indicators are defined in the OpenTender schema. |
| `tender_indicator_TRANSPARENCY_BIDDER_NAME_MISSING` | Tender-level indicator score for \`TRANSPARENCY\_BIDDER\_NAME\_MISSING\` (0–100 when computed, empty otherwise). Integrity, transparency, and administrative indicators are defined in the OpenTender schema. |
| `tender_indicator_TRANSPARENCY_MARKET_MISSING` | Tender-level indicator score for \`TRANSPARENCY\_MARKET\_MISSING\` (0–100 when computed, empty otherwise). Integrity, transparency, and administrative indicators are defined in the OpenTender schema. |
| `tender_indicator_TRANSPARENCY_TITLE_MISSING` | Tender-level indicator score for \`TRANSPARENCY\_TITLE\_MISSING\` (0–100 when computed, empty otherwise). Integrity, transparency, and administrative indicators are defined in the OpenTender schema. |
| `tender_indicator_TRANSPARENCY_VALUE_MISSING` | Tender-level indicator score for \`TRANSPARENCY\_VALUE\_MISSING\` (0–100 when computed, empty otherwise). Integrity, transparency, and administrative indicators are defined in the OpenTender schema. |
| `tender_indicator_TRANSPARENCY_YEAR_MISSING` | Tender-level indicator score for \`TRANSPARENCY\_YEAR\_MISSING\` (0–100 when computed, empty otherwise). Integrity, transparency, and administrative indicators are defined in the OpenTender schema. |
| `tender_indicator_INTEGRITY_WINNER_CA_SHARE` | Tender-level indicator score for \`INTEGRITY\_WINNER\_CA\_SHARE\` (0–100 when computed, empty otherwise). Integrity, transparency, and administrative indicators are defined in the OpenTender schema. |
| `tender_indicator_INTEGRITY_NR_OF_REQUESTED_BIDS` | Tender-level indicator score for \`INTEGRITY\_NR\_OF\_REQUESTED\_BIDS\` (0–100 when computed, empty otherwise). Integrity, transparency, and administrative indicators are defined in the OpenTender schema. |
| `tender_indicator_TRANSPARENCY_CA_TYPE_MISSING` | Tender-level indicator score for \`TRANSPARENCY\_CA\_TYPE\_MISSING\` (0–100 when computed, empty otherwise). Integrity, transparency, and administrative indicators are defined in the OpenTender schema. |
| `tender_indicator_TRANSPARENCY_IMP_LOC_MISSING` | Tender-level indicator score for \`TRANSPARENCY\_IMP\_LOC\_MISSING\` (0–100 when computed, empty otherwise). Integrity, transparency, and administrative indicators are defined in the OpenTender schema. |
| `tender_indicator_TRANSPARENCY_BID_NR_MISSING` | Tender-level indicator score for \`TRANSPARENCY\_BID\_NR\_MISSING\` (0–100 when computed, empty otherwise). Integrity, transparency, and administrative indicators are defined in the OpenTender schema. |
| `tender_indicator_TRANSPARENCY_HEADOFENTITY_DATE_MISSING` | Tender-level indicator score for \`TRANSPARENCY\_HEADOFENTITY\_DATE\_MISSING\` (0–100 when computed, empty otherwise). Integrity, transparency, and administrative indicators are defined in the OpenTender schema. |
| `tender_indicator_TRANSPARENCY_COMMITTEE_DATE_MISSING` | Tender-level indicator score for \`TRANSPARENCY\_COMMITTEE\_DATE\_MISSING\` (0–100 when computed, empty otherwise). Integrity, transparency, and administrative indicators are defined in the OpenTender schema. |
| `tender_indicator_TRANSPARENCY_MISSING_OR_INCOMPLETE_AWARD_CRITERIA` | Tender-level indicator score for \`TRANSPARENCY\_MISSING\_OR\_INCOMPLETE\_AWARD\_CRITERIA\` (0–100 when computed, empty otherwise). Integrity, transparency, and administrative indicators are defined in the OpenTender schema. |
| `tender_indicator_TRANSPARENCY_MISSING_ELIGIBLE_BID_LANGUAGES` | Tender-level indicator score for \`TRANSPARENCY\_MISSING\_ELIGIBLE\_BID\_LANGUAGES\` (0–100 when computed, empty otherwise). Integrity, transparency, and administrative indicators are defined in the OpenTender schema. |
| `tender_indicator_TRANSPARENCY_MISSING_OR_INCOMPLETE_FUNDINGS_INFO` | Tender-level indicator score for \`TRANSPARENCY\_MISSING\_OR\_INCOMPLETE\_FUNDINGS\_INFO\` (0–100 when computed, empty otherwise). Integrity, transparency, and administrative indicators are defined in the OpenTender schema. |
| `tender_indicator_TRANSPARENCY_MISSING_OR_INCOMPLETE_DURATION_INFO` | Tender-level indicator score for \`TRANSPARENCY\_MISSING\_OR\_INCOMPLETE\_DURATION\_INFO\` (0–100 when computed, empty otherwise). Integrity, transparency, and administrative indicators are defined in the OpenTender schema. |
| `tender_indicator_TRANSPARENCY_MISSING_SUBCONTRACTED_INFO` | Tender-level indicator score for \`TRANSPARENCY\_MISSING\_SUBCONTRACTED\_INFO\` (0–100 when computed, empty otherwise). Integrity, transparency, and administrative indicators are defined in the OpenTender schema. |
| `tender_indicator_TRANSPARENCY_MISSING_ADDRESS_OF_IMPLEMENTATION_NUTS` | Tender-level indicator score for \`TRANSPARENCY\_MISSING\_ADDRESS\_OF\_IMPLEMENTATION\_NUTS\` (0–100 when computed, empty otherwise). Integrity, transparency, and administrative indicators are defined in the OpenTender schema. |
| `tender_indicator_TRANSPARENCY_MISSING_OR_INCOMPLETE_CPVS` | Tender-level indicator score for \`TRANSPARENCY\_MISSING\_OR\_INCOMPLETE\_CPVS\` (0–100 when computed, empty otherwise). Integrity, transparency, and administrative indicators are defined in the OpenTender schema. |
| `tender_indicator_TRANSPARENCY_MISSING_SELECTION_METHOD` | Tender-level indicator score for \`TRANSPARENCY\_MISSING\_SELECTION\_METHOD\` (0–100 when computed, empty otherwise). Integrity, transparency, and administrative indicators are defined in the OpenTender schema. |
| `tender_indicator_TRANSPARENCY_SIGN_DATE_MISSING` | Tender-level indicator score for \`TRANSPARENCY\_SIGN\_DATE\_MISSING\` (0–100 when computed, empty otherwise). Integrity, transparency, and administrative indicators are defined in the OpenTender schema. |
| `tender_indicator_INTEGRITY_CONTRACT_MODIFICATION` | Tender-level indicator score for \`INTEGRITY\_CONTRACT\_MODIFICATION\` (0–100 when computed, empty otherwise). Integrity, transparency, and administrative indicators are defined in the OpenTender schema. |
| `tender_indicator_INTEGRITY_MULTIPLE_VALID_BIDS` | Tender-level indicator score for \`INTEGRITY\_MULTIPLE\_VALID\_BIDS\` (0–100 when computed, empty otherwise). Integrity, transparency, and administrative indicators are defined in the OpenTender schema. |
| `tender_indicator_INTEGRITY_TEXT_BASED_OPENNESS` | Tender-level indicator score for \`INTEGRITY\_TEXT\_BASED\_OPENNESS\` (0–100 when computed, empty otherwise). Integrity, transparency, and administrative indicators are defined in the OpenTender schema. |

## Buyer (contracting authority)

| Column | Description |
| :--- | :--- |
| `buyer_row_nr` | Row index for the buyer on this CSV line (1-based within buyer list for the tender). |
| `buyer_buyerType` | Type of contracting authority when coded. |
| `buyer_mainActivities` | Comma-separated main activity codes. |
| `buyer_bodyIds` | Comma-separated external body identifiers linked to the buyer. |
| `buyer_id` | OpenTender buyer / authority organisation id. |
| `buyer_name` | Buyer legal name. |
| `buyer_nuts` | Comma-separated NUTS codes from the buyer address. |
| `buyer_city` | Buyer city. |
| `buyer_country` | Buyer country code from address. |
| `buyer_postcode` | Buyer postcode. |

## Lot

| Column | Description |
| :--- | :--- |
| `lot_row_nr` | Row index for the lot on this CSV line (1-based within lots for the tender). |
| `lot_title` | Lot title. |
| `lot_selectionMethod` | Lot selection / award method when present. |
| `lot_lotId` | Lot identifier from source notice. |
| `lot_status` | Lot status string. |
| `lot_estimatedCompletionDate` | Lot estimated completion date (YYYY/MM/DD). |
| `lot_estimatedStartDate` | Lot estimated start date (YYYY/MM/DD). |
| `lot_contractSignatureDate` | Lot-level contract signature date (YYYY/MM/DD). |
| `lot_cancellationDate` | Lot cancellation date (YYYY/MM/DD). |
| `lot_isAwarded` | Lot awarded (\`yes\` / \`no\`). |
| `lot_estimatedPrice` | Lot estimated net amount in national currency. |
| `lot_estimatedPrice_currency` | Lot estimated national currency code. |
| `lot_estimatedPrice_minNetAmount` | Lot estimated minimum net amount. |
| `lot_estimatedPrice_maxNetAmount` | Lot estimated maximum net amount. |
| `lot_estimatedPrice_EUR` | Lot estimated net amount in EUR. |
| `lot_lotNumber` | Sequential lot number when provided. |
| `lot_bidsCount` | Total bids received for the lot. |
| `lot_validBidsCount` | Valid bids count. |
| `lot_smeBidsCount` | Bids from SMEs count. |
| `lot_electronicBidsCount` | Electronic bids count. |
| `lot_nonEuMemberStatesCompaniesBidsCount` | Bids from non-EU companies count. |
| `lot_otherEuMemberStatesCompaniesBidsCount` | Bids from other EU member state companies count. |
| `lot_foreignCompaniesBidsCount` | Bids from foreign companies count. |
| `lot_description` | Lot description text. |
| `lot_description_length` | Character length of lot description. |
| `lot_fundingProgrammes` | Comma-separated funding programmes on the lot. |
| `lot_addressOfImplementation_nuts` | Comma-separated NUTS for lot place of performance. |
| `lot_addressOfImplementation_country` | Country for lot place of performance. |
| `lot_addressOfImplementation_postcode` | Postcode for lot place of performance. |
| `lot_addressOfImplementation_city` | City for lot place of performance (trailing space in header matches export). |
| `lot_addressOfImplementation_street` | Street for lot place of performance (trailing space in header matches export). |
| `lot_indicator_metadata_decisionPeriodLength` | Comma-separated decision-period metadata from lot-level indicators when present. |
| `lot_indicator_metadata_bidderGroupId` | Comma-separated bidder-group identifiers from lot-level indicator metadata. |

## Bid

| Column | Description |
| :--- | :--- |
| `bid_row_nr` | Row index for the bid on this CSV line (1-based within bids for the lot). |
| `bid_isWinning` | This bid is the winner (\`yes\` / \`no\`). |
| `bid_isSubcontracted` | Bid involves subcontracting (\`yes\` / \`no\`). |
| `bid_isConsortium` | Consortium bid (\`yes\` / \`no\`). |
| `bid_subcontractedProportion` | Subcontracted proportion when provided (numeric). |
| `bid_price` | Bid net amount in national currency. |
| `bid_price_currency` | Bid national currency code. |
| `bid_price_minNetAmount` | Bid minimum net amount when a range is used. |
| `bid_price_maxNetAmount` | Bid maximum net amount when a range is used. |
| `bid_price_EUR` | Bid net amount in EUR. |
| `bid_subcontractedValue_netAmountEur` | Subcontracted value in EUR when available. |
| `bid_subcontractedName` | Name of subcontractor when a single name field is stored. |

## Bidder (company)

| Column | Description |
| :--- | :--- |
| `bidder_row_nr` | Row index for the bidder on this CSV line (1-based within bidders for the bid). |
| `bidder_bodyIds` | Comma-separated external body identifiers for the bidder. |
| `bidder_id` | OpenTender company / bidder organisation id. |
| `bidder_name` | Bidder legal name. |
| `bidder_nuts` | Comma-separated NUTS from bidder address. |
| `bidder_city` | Bidder city. |
| `bidder_country` | Bidder country from address. |
| `bidder_postcode` | Bidder postcode. |
