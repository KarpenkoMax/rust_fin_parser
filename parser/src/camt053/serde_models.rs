use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct Camt053Entry {
    #[serde(rename = "Amt")]
    pub(crate) amount: CamtAmtXml,

    #[serde(rename = "CdtDbtInd")]
    pub(crate) cdt_dbt_ind: String,

    #[serde(rename = "BookgDt")]
    pub(crate) booking_date: CamtDateXml,

    #[serde(rename = "ValDt")]
    pub(crate) value_date: CamtDateXml,

    #[serde(rename = "NtryDtls")]
    pub(crate) details: Option<CamtEntryDetails>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct Camt053Statement {
    /// <Id>...</Id> - идентификатор выписки (может быть None)
    #[serde(rename = "Id")]
    pub(crate) id: Option<String>,

    /// <ElctrncSeqNb>1</ElctrncSeqNb>
    #[serde(rename = "ElctrncSeqNb")]
    pub(crate) sequence_number: Option<u32>,

    /// <CreDtTm>2023-04-20T23:24:31</CreDtTm>
    #[serde(rename = "CreDtTm")]
    pub(crate) created_at: Option<String>,

    /// <FrToDt>...</FrToDt>
    #[serde(rename = "FrToDt")]
    pub(crate) period: Option<Camt053Period>,

    /// <Acct>...</Acct>
    #[serde(rename = "Acct")]
    pub(crate) account: Camt053Account,

    /// Все <Bal>...</Bal>
    #[serde(rename = "Bal", default)]
    pub(crate) balances: Vec<Camt053Balance>,

    /// Все <Ntry>...</Ntry>
    #[serde(rename = "Ntry", default)]
    pub(crate) entries: Vec<Camt053Entry>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename = "Document")]
pub(crate) struct Camt053Document {
    /// <BkToCstmrStmt>...</BkToCstmrStmt>
    #[serde(rename = "BkToCstmrStmt")]
    pub(crate) bank_to_customer: Camt053BankToCustomer,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct Camt053BankToCustomer {
    /// <GrpHdr>...</GrpHdr>
    #[serde(rename = "GrpHdr")]
    pub(crate) group_header: Option<Camt053GroupHeader>,

    /// <Stmt>...</Stmt>
    #[serde(rename = "Stmt", default)]
    pub(crate) statements: Vec<Camt053Statement>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct Camt053GroupHeader {
    /// <MsgId>...</MsgId>
    #[serde(rename = "MsgId")]
    pub(crate) message_id: String,

    /// <CreDtTm>2023-04-20T23:24:31</CreDtTm>
    #[serde(rename = "CreDtTm")]
    pub(crate) created_at: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct CamtAmtXml {
    #[serde(rename = "@Ccy")]
    pub(crate) currency: String,

    #[serde(rename = "$text")]
    pub(crate) value: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct CamtDateXml {
    #[serde(rename = "Dt")]
    pub(crate) date: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct CamtRefs {
    // EndToEndId TxId InstrId PmtInfId
    #[serde(rename = "EndToEndId")]
    pub(crate) end_to_end_id: Option<String>,

    #[serde(rename = "TxId")]
    pub(crate) tx_id: Option<String>,

    #[serde(rename = "InstrId")]
    pub(crate) instr_id: Option<String>,

    #[serde(rename = "PmtInfId")]
    pub(crate) pmt_inf_id: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct CamtAmountDetails {
    #[serde(rename = "InstdAmt")]
    pub(crate) instructed: Option<CamtInstructedAmount>,

    #[serde(rename = "TxAmt")]
    pub(crate) transaction: Option<CamtTransactionAmount>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct CamtInstructedAmount {
    #[serde(rename = "Amt")]
    pub(crate) amount: CamtMoney,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct CamtTransactionAmount {
    #[serde(rename = "Amt")]
    pub(crate) amount: CamtMoney,

    #[serde(rename = "CcyXchg")]
    pub(crate) fx: Option<CamtCurrencyExchange>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct CamtMoney {
    /// Атрибут Ccy="EUR"/"DKK"
    #[serde(rename = "@Ccy")]
    pub(crate) currency: String,

    #[serde(rename = "$text")]
    pub(crate) value: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct CamtCurrencyExchange {
    #[serde(rename = "SrcCcy")]
    pub(crate) src_ccy: Option<String>, // EUR

    #[serde(rename = "TrgtCcy")]
    pub(crate) trgt_ccy: Option<String>, // DKK

    #[serde(rename = "UnitCcy")]
    pub(crate) unit_ccy: Option<String>, // EUR

    #[serde(rename = "XchgRate")]
    pub(crate) rate: Option<String>, // "7.4738000"
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct CamtRelatedParties {
    /// <Dbtr>
    #[serde(rename = "Dbtr", skip_serializing_if = "Option::is_none")]
    pub(crate) debtor: Option<CamtParty>,

    /// <DbtrAcct>
    #[serde(rename = "DbtrAcct", skip_serializing_if = "Option::is_none")]
    pub(crate) debtor_account: Option<CamtAccount>,

    /// <Cdtr>
    #[serde(rename = "Cdtr", skip_serializing_if = "Option::is_none")]
    pub(crate) creditor: Option<CamtParty>,

    /// <CdtrAcct>
    #[serde(rename = "CdtrAcct", skip_serializing_if = "Option::is_none")]
    pub(crate) creditor_account: Option<CamtAccount>,

    /// <UltmtDbtr>
    #[serde(rename = "UltmtDbtr", skip_serializing_if = "Option::is_none")]
    pub(crate) ultimate_debtor: Option<CamtParty>,

    /// <UltmtCdtr>
    #[serde(rename = "UltmtCdtr", skip_serializing_if = "Option::is_none")]
    pub(crate) ultimate_creditor: Option<CamtParty>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct CamtParty {
    /// <Nm>
    #[serde(rename = "Nm")]
    pub(crate) name: Option<String>,

    /// <PstlAdr>
    #[serde(rename = "PstlAdr")]
    pub(crate) postal_address: Option<CamtPostalAddress>,

    /// <Id>
    #[serde(rename = "Id")]
    pub(crate) id: Option<CamtPartyId>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct CamtAccount {
    #[serde(rename = "Id")]
    pub(crate) id: CamtAccountId,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct CamtAccountId {
    /// <IBAN>
    #[serde(rename = "IBAN")]
    pub(crate) iban: Option<String>,
}

/// Пока можно сделать очень простой адрес
#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct CamtPostalAddress {
    #[serde(rename = "StrtNm")]
    pub(crate) street: Option<String>,

    #[serde(rename = "PstCdId")]
    pub(crate) postcode: Option<String>,

    #[serde(rename = "TwnNm")]
    pub(crate) town: Option<String>,

    #[serde(rename = "Ctry")]
    pub(crate) country: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct CamtPartyId {}

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct CamtRemittanceInfo {
    /// <Ustrd>
    #[serde(rename = "Ustrd", default)]
    pub(crate) unstructured: Vec<String>,

    /// <Strd>
    #[serde(rename = "Strd", default)]
    pub(crate) structured: Vec<CamtStructuredRemittance>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct CamtStructuredRemittance {}

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct CamtRelatedDates {
    /// <AccptncDtTm>
    #[serde(rename = "AccptncDtTm")]
    pub(crate) acceptance_datetime: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct CamtTxDtls {
    #[serde(rename = "Refs")]
    pub(crate) refs: Option<CamtRefs>,

    #[serde(rename = "AmtDtls")]
    pub(crate) amount_details: Option<CamtAmountDetails>,

    #[serde(rename = "RltdPties")]
    pub(crate) related_parties: Option<CamtRelatedParties>,

    #[serde(rename = "RmtInf")]
    pub(crate) rmt_inf: Option<CamtRemittanceInfo>,

    #[serde(rename = "RltdDts")]
    pub(crate) related_datetimes: Option<CamtRelatedDates>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct CamtEntryDetails {
    #[serde(rename = "TxDtls")]
    pub(crate) tx_details: Vec<CamtTxDtls>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct Camt053Account {
    /// <Acct><Id>
    #[serde(rename = "Id")]
    pub(crate) id: Camt053AccountId,

    /// <Acct><Nm>
    #[serde(rename = "Nm")]
    pub(crate) name: Option<String>,

    /// <Acct><Ccy>DKK</Ccy></Acct>
    #[serde(rename = "Ccy")]
    pub(crate) currency: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct Camt053AccountId {
    /// <IBAN>
    #[serde(rename = "IBAN")]
    pub(crate) iban: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct Camt053Period {
    /// <FrToDt><FrDtTm>...</FrDtTm></FrToDt>
    #[serde(rename = "FrDtTm")]
    pub(crate) from: Option<String>,

    /// <FrToDt><ToDtTm>...</ToDtTm></FrToDt>
    #[serde(rename = "ToDtTm")]
    pub(crate) to: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct Camt053Balance {
    /// Тип баланса (OPBD / CLBD / ...).
    #[serde(rename = "Tp")]
    pub(crate) balance_type: Camt053BalanceType,

    /// <Amt Ccy="DKK">360000.00</Amt>
    #[serde(rename = "Amt")]
    pub(crate) amount: CamtAmtXml,

    /// <CdtDbtInd>CRDT</CdtDbtInd>
    #[serde(rename = "CdtDbtInd")]
    pub(crate) cdt_dbt_ind: Option<String>,

    /// <Dt><Dt>2023-04-19</Dt></Dt>
    #[serde(rename = "Dt", default, skip_serializing_if = "Option::is_none")]
    pub(crate) date: Option<CamtDateXml>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct Camt053BalanceType {
    /// <Tp><CdOrPrtry><Cd>OPBD</Cd></CdOrPrtry></Tp>
    #[serde(rename = "CdOrPrtry")]
    pub(crate) code_or_proprietary: Camt053BalanceCodeOrProprietary,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct Camt053BalanceCodeOrProprietary {
    /// <Cd>OPBD</Cd> / <Cd>CLBD</Cd> и т.п.
    #[serde(rename = "Cd")]
    pub(crate) code: Option<String>,
}
