use serde::{Serialize, Deserialize};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Camt053Entry {
    #[serde(rename = "Amt")]
    pub amount: CamtAmtXml,

    #[serde(rename = "CdtDbtInd")]
    pub cdt_dbt_ind: String,

    #[serde(rename = "BookgDt")]
    pub booking_date: CamtDateXml,

    #[serde(rename = "ValDt")]
    pub value_date: CamtDateXml,

    #[serde(rename = "NtryDtls")]
    pub details: Option<CamtEntryDetails>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Camt053Statement {
    /// <Id>...</Id> — идентификатор выписки (может быть None)
    #[serde(rename = "Id")]
    pub id: Option<String>,

    /// <ElctrncSeqNb>1</ElctrncSeqNb>
    #[serde(rename = "ElctrncSeqNb")]
    pub sequence_number: Option<u32>,

    /// <CreDtTm>2023-04-20T23:24:31</CreDtTm>
    #[serde(rename = "CreDtTm")]
    pub created_at: Option<String>,

    /// <FrToDt>...</FrToDt>
    #[serde(rename = "FrToDt")]
    pub period: Option<Camt053Period>,

    /// <Acct>...</Acct>
    #[serde(rename = "Acct")]
    pub account: Camt053Account,

    /// Все <Bal>...</Bal>
    #[serde(rename = "Bal", default)]
    pub balances: Vec<Camt053Balance>,

    /// Все <Ntry>...</Ntry>
    #[serde(rename = "Ntry", default)]
    pub entries: Vec<Camt053Entry>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename = "Document")]
pub struct Camt053Document {
    /// <BkToCstmrStmt>...</BkToCstmrStmt>
    #[serde(rename = "BkToCstmrStmt")]
    pub bank_to_customer: Camt053BankToCustomer,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Camt053BankToCustomer {
    /// <GrpHdr>...</GrpHdr>
    #[serde(rename = "GrpHdr")]
    pub group_header: Option<Camt053GroupHeader>,

    /// <Stmt>...</Stmt>
    #[serde(rename = "Stmt", default)]
    pub statements: Vec<Camt053Statement>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Camt053GroupHeader {
    /// <MsgId>...</MsgId>
    #[serde(rename = "MsgId")]
    pub message_id: String,

    /// <CreDtTm>2023-04-20T23:24:31</CreDtTm>
    #[serde(rename = "CreDtTm")]
    pub created_at: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CamtAmtXml {
    #[serde(rename = "@Ccy")]
    pub currency: String,

    #[serde(rename = "$text")]
    pub value: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CamtDateXml {
    #[serde(rename = "Dt")]
    pub date: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CamtRefs {
    // EndToEndId TxId InstrId PmtInfId
    #[serde(rename = "EndToEndId")]
    pub end_to_end_id: Option<String>,

    #[serde(rename = "TxId")]
    pub tx_id: Option<String>,

    #[serde(rename = "InstrId")]
    pub instr_id: Option<String>,

    #[serde(rename = "PmtInfId")]
    pub pmt_inf_id: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CamtAmountDetails {
    #[serde(rename = "InstdAmt")]
    pub instructed: Option<CamtInstructedAmount>,

    #[serde(rename = "TxAmt")]
    pub transaction: Option<CamtTransactionAmount>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CamtInstructedAmount {
    #[serde(rename = "Amt")]
    pub amount: CamtMoney,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CamtTransactionAmount {
    #[serde(rename = "Amt")]
    pub amount: CamtMoney,

    #[serde(rename = "CcyXchg")]
    pub fx: Option<CamtCurrencyExchange>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CamtMoney {
    /// Атрибут Ccy="EUR"/"DKK"
    #[serde(rename = "@Ccy")]
    pub currency: String,

    #[serde(rename = "$text")]
    pub value: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CamtCurrencyExchange {
    #[serde(rename = "SrcCcy")]
    pub src_ccy: Option<String>,   // EUR

    #[serde(rename = "TrgtCcy")]
    pub trgt_ccy: Option<String>,  // DKK

    #[serde(rename = "UnitCcy")]
    pub unit_ccy: Option<String>,  // EUR

    #[serde(rename = "XchgRate")]
    pub rate: Option<String>,      // "7.4738000"
}


#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CamtRelatedParties {
    /// <Dbtr>
    #[serde(rename = "Dbtr")]
    pub debtor: Option<CamtParty>,

    /// <DbtrAcct>
    #[serde(rename = "DbtrAcct")]
    pub debtor_account: Option<CamtAccount>,

    /// <Cdtr>
    #[serde(rename = "Cdtr")]
    pub creditor: Option<CamtParty>,

    /// <CdtrAcct>
    #[serde(rename = "CdtrAcct")]
    pub creditor_account: Option<CamtAccount>,

    /// <UltmtDbtr>
    #[serde(rename = "UltmtDbtr")]
    pub ultimate_debtor: Option<CamtParty>,

    /// <UltmtCdtr>
    #[serde(rename = "UltmtCdtr")]
    pub ultimate_creditor: Option<CamtParty>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CamtParty {
    /// <Nm>
    #[serde(rename = "Nm")]
    pub name: Option<String>,

    /// <PstlAdr>
    #[serde(rename = "PstlAdr")]
    pub postal_address: Option<CamtPostalAddress>,

    /// <Id>
    #[serde(rename = "Id")]
    pub id: Option<CamtPartyId>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CamtAccount {
    #[serde(rename = "Id")]
    pub id: CamtAccountId,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CamtAccountId {
    /// <IBAN>
    #[serde(rename = "IBAN")]
    pub iban: Option<String>,
}

/// Пока можно сделать очень простой адрес
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CamtPostalAddress {
    #[serde(rename = "StrtNm")]
    pub street: Option<String>,

    #[serde(rename = "PstCdId")]
    pub postcode: Option<String>,

    #[serde(rename = "TwnNm")]
    pub town: Option<String>,

    #[serde(rename = "Ctry")]
    pub country: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CamtPartyId {}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CamtRemittanceInfo {
    /// <Ustrd>
    #[serde(rename = "Ustrd", default)]
    pub unstructured: Vec<String>,

    /// <Strd>
    #[serde(rename = "Strd", default)]
    pub structured: Vec<CamtStructuredRemittance>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CamtStructuredRemittance {
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CamtRelatedDates {
    /// <AccptncDtTm>
    #[serde(rename = "AccptncDtTm")]
    pub acceptance_datetime: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CamtTxDtls {
    #[serde(rename = "Refs")]
    pub refs: Option<CamtRefs>,

    #[serde(rename = "AmtDtls")]
    pub amount_details: Option<CamtAmountDetails>,

    #[serde(rename = "RltdPties")]
    pub related_parties: Option<CamtRelatedParties>,

    #[serde(rename = "RmtInf")]
    pub rmt_inf: Option<CamtRemittanceInfo>,

    #[serde(rename = "RltdDts")]
    pub related_datetimes: Option<CamtRelatedDates>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CamtEntryDetails {
    #[serde(rename = "TxDtls")]
    pub tx_details: Vec<CamtTxDtls>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Camt053Account {
    /// <Acct><Id>
    #[serde(rename = "Id")]
    pub id: Camt053AccountId,

    /// <Acct><Nm>
    #[serde(rename = "Nm")]
    pub name: Option<String>,

    /// <Acct><Ccy>DKK</Ccy></Acct>
    #[serde(rename = "Ccy")]
    pub currency: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Camt053AccountId {
    /// <IBAN>
    #[serde(rename = "IBAN")]
    pub iban: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Camt053Period {
    /// <FrToDt><FrDtTm>...</FrDtTm></FrToDt>
    #[serde(rename = "FrDtTm")]
    pub from: Option<String>,

    /// <FrToDt><ToDtTm>...</ToDtTm></FrToDt>
    #[serde(rename = "ToDtTm")]
    pub to: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Camt053Balance {
    /// Тип баланса (OPBD / CLBD / ...).
    #[serde(rename = "Tp")]
    pub balance_type: Camt053BalanceType,

    /// <Amt Ccy="DKK">360000.00</Amt>
    #[serde(rename = "Amt")]
    pub amount: CamtAmtXml,

    /// <CdtDbtInd>CRDT</CdtDbtInd>
    #[serde(rename = "CdtDbtInd")]
    pub cdt_dbt_ind: Option<String>,

    /// <Dt><Dt>2023-04-19</Dt></Dt>
    #[serde(rename = "Dt")]
    pub date: Option<CamtDateXml>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Camt053BalanceType {
    /// <Tp><CdOrPrtry><Cd>OPBD</Cd></CdOrPrtry></Tp>
    #[serde(rename = "CdOrPrtry")]
    pub code_or_proprietary: Camt053BalanceCodeOrProprietary,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Camt053BalanceCodeOrProprietary {
    /// <Cd>OPBD</Cd> / <Cd>CLBD</Cd> и т.п.
    #[serde(rename = "Cd")]
    pub code: Option<String>,
}


