//! Determinism: BitwisePortable
//! Synthetic `franken_accounting`-compatible local mining ledger (`BD-134`, plan §21.3, §33.3).
//!
//! Uses exact `fmine_units::Money` (`i128` minor units, zero floating-point rounding) to generate
//! double-entry monthly mining postings (`fa:external-subledger-detail:v1` shape), verify exact
//! trial-balance closure (`Σ Debits == Σ Credits` per currency), and compute deterministic
//! Merkle-folded posting detail digests (`SHA-256` over sorted detail lines).

use std::collections::BTreeMap;

use fmine_core::codec::Encoder;
use fmine_core::hash::{Digest32, sha256};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_units::{Currency, Money};

/// Mining chart-of-accounts entries aligned to the §21.3 domain pack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MiningAccount {
    Cash,
    OreStockpileInventory,
    WipMillInventory,
    RefinedBullionInventory,
    DeferredStrippingAsset,
    RoyaltyPayable,
    RehabilitationProvision,
    RetainedEquity,
    DoreSalesRevenue,
    MiningOperatingExpense,
    ProcessingExpense,
    DepreciationExpense,
    RoyaltyExpense,
}

impl MiningAccount {
    /// Canonical account code matching FA external subledger posting slots.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Cash => "1010-CASH",
            Self::OreStockpileInventory => "1210-INV-ORE",
            Self::WipMillInventory => "1220-INV-WIP",
            Self::RefinedBullionInventory => "1230-INV-BULLION",
            Self::DeferredStrippingAsset => "1510-DEF-STRIP",
            Self::RoyaltyPayable => "2110-ROYALTY-PAY",
            Self::RehabilitationProvision => "2410-REHAB-PROV",
            Self::RetainedEquity => "3100-EQUITY",
            Self::DoreSalesRevenue => "4100-REV-DORE",
            Self::MiningOperatingExpense => "5110-OPEX-MINE",
            Self::ProcessingExpense => "5120-OPEX-MILL",
            Self::DepreciationExpense => "5210-DA",
            Self::RoyaltyExpense => "5310-ROYALTY",
        }
    }
}

/// Debit or credit leg of a double-entry posting line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EntrySide {
    Debit,
    Credit,
}

/// One immutable line inside a double-entry journal posting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostingLine {
    pub account: MiningAccount,
    pub side: EntrySide,
    pub amount: Money,
    pub memo: String,
}

/// One balanced double-entry journal entry (`fa:external-subledger-detail:v1`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalEntry {
    pub entry_id: String,
    pub period: String,
    pub site_id: String,
    pub lines: Vec<PostingLine>,
    /// Deterministic SHA-256 Merkle-folded digest over canonical sorted posting lines.
    pub detail_digest: Digest32,
}

impl JournalEntry {
    /// Build a verified double-entry posting, refusing unbalanced debits/credits (`FMINE-REF-101`)
    /// or non-positive line amounts (`FMINE-REF-109`).
    pub fn build(
        entry_id: impl Into<String>,
        period: impl Into<String>,
        site_id: impl Into<String>,
        lines: Vec<PostingLine>,
    ) -> Result<Self, Refusal> {
        let entry_id = entry_id.into();
        let period = period.into();
        let site_id = site_id.into();
        if lines.len() < 2 {
            return Err(Refusal::new(
                RefusalCode::Conservation101,
                format!("entry '{entry_id}' needs at least 2 posting lines"),
            ));
        }
        let mut net_by_ccy: BTreeMap<Currency, i128> = BTreeMap::new();
        for line in &lines {
            if line.amount.minor <= 0 {
                return Err(Refusal::new(
                    RefusalCode::OutsidePhysicalDomain109,
                    format!(
                        "entry '{entry_id}' line on {} has non-positive amount {}",
                        line.account.code(),
                        line.amount.minor
                    ),
                ));
            }
            let signed = match line.side {
                EntrySide::Debit => line.amount.minor,
                EntrySide::Credit => -line.amount.minor,
            };
            let cur = net_by_ccy.entry(line.amount.ccy).or_insert(0);
            *cur = cur.checked_add(signed).ok_or_else(|| {
                Refusal::new(
                    RefusalCode::OutsidePhysicalDomain109,
                    format!("money overflow in entry '{entry_id}'"),
                )
            })?;
        }
        for (ccy, net) in &net_by_ccy {
            if *net != 0 {
                return Err(Refusal::new(
                    RefusalCode::Conservation101,
                    format!(
                        "entry '{entry_id}' unbalanced in {ccy:?}: debits - credits = {net} minor units"
                    ),
                ));
            }
        }

        // Canonical Merkle-folded detail digest over sorted lines.
        let mut sorted: Vec<&PostingLine> = lines.iter().collect();
        sorted.sort_by(|a, b| {
            (a.account, a.side, a.amount.ccy, a.amount.minor, &a.memo).cmp(&(
                b.account,
                b.side,
                b.amount.ccy,
                b.amount.minor,
                &b.memo,
            ))
        });
        let mut enc = Encoder::new_frame(*b"POST", 1);
        enc.put_str(&entry_id);
        enc.put_str(&period);
        enc.put_str(&site_id);
        enc.put_u32(sorted.len() as u32);
        for line in sorted {
            enc.put_str(line.account.code());
            enc.put_u8(match line.side {
                EntrySide::Debit => 1,
                EntrySide::Credit => 2,
            });
            enc.put_i128(line.amount.minor);
            enc.put_str(&line.memo);
        }
        let detail_digest = sha256(&enc.finish());

        Ok(Self {
            entry_id,
            period,
            site_id,
            lines,
            detail_digest,
        })
    }
}

/// Exact local trial balance over all posted [`JournalEntry`] items (`BD-134`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalAccountingBook {
    pub book_id: String,
    pub currency: Currency,
    pub entries: Vec<JournalEntry>,
    /// Net debit-minus-credit balance per account in exact minor units.
    pub account_balances: BTreeMap<MiningAccount, i128>,
    pub total_debits: i128,
    pub total_credits: i128,
    pub book_root_digest: Digest32,
}

impl LocalAccountingBook {
    /// Post a sequence of verified journal entries and assert exact trial-balance closure.
    pub fn build(
        book_id: impl Into<String>,
        currency: Currency,
        entries: Vec<JournalEntry>,
    ) -> Result<Self, Refusal> {
        let book_id = book_id.into();
        let mut account_balances: BTreeMap<MiningAccount, i128> = BTreeMap::new();
        let mut total_debits = 0_i128;
        let mut total_credits = 0_i128;

        let mut enc = Encoder::new_frame(*b"BOOK", 1);
        enc.put_str(&book_id);
        enc.put_u32(entries.len() as u32);

        for entry in &entries {
            enc.put_fixed(entry.detail_digest.as_bytes());
            for line in &entry.lines {
                if line.amount.ccy != currency {
                    return Err(Refusal::new(
                        RefusalCode::BasisMismatch105,
                        format!(
                            "entry '{}' uses {:?} inside {:?} book '{}'",
                            entry.entry_id, line.amount.ccy, currency, book_id
                        ),
                    ));
                }
                match line.side {
                    EntrySide::Debit => {
                        total_debits =
                            total_debits.checked_add(line.amount.minor).ok_or_else(|| {
                                Refusal::new(
                                    RefusalCode::OutsidePhysicalDomain109,
                                    "total_debits overflow",
                                )
                            })?;
                        let bal = account_balances.entry(line.account).or_insert(0);
                        *bal = bal.checked_add(line.amount.minor).ok_or_else(|| {
                            Refusal::new(
                                RefusalCode::OutsidePhysicalDomain109,
                                "account balance overflow",
                            )
                        })?;
                    }
                    EntrySide::Credit => {
                        total_credits =
                            total_credits
                                .checked_add(line.amount.minor)
                                .ok_or_else(|| {
                                    Refusal::new(
                                        RefusalCode::OutsidePhysicalDomain109,
                                        "total_credits overflow",
                                    )
                                })?;
                        let bal = account_balances.entry(line.account).or_insert(0);
                        *bal = bal.checked_sub(line.amount.minor).ok_or_else(|| {
                            Refusal::new(
                                RefusalCode::OutsidePhysicalDomain109,
                                "account balance overflow",
                            )
                        })?;
                    }
                }
            }
        }

        let trial_sum: i128 = account_balances.values().copied().sum();
        if total_debits != total_credits || trial_sum != 0 {
            return Err(Refusal::new(
                RefusalCode::Conservation101,
                format!(
                    "book '{book_id}' trial balance open: debits={total_debits}, credits={total_credits}, net={trial_sum}"
                ),
            ));
        }

        Ok(Self {
            book_id,
            currency,
            entries,
            account_balances,
            total_debits,
            total_credits,
            book_root_digest: sha256(&enc.finish()),
        })
    }
}

/// Generate a complete multi-period synthetic mining book (`BD-134`) with exact USD balances.
pub fn generate_synthetic_mining_book(
    book_id: &str,
    site_id: &str,
    periods: usize,
) -> Result<LocalAccountingBook, Refusal> {
    let usd = |dollars: i128| Money {
        ccy: Currency::USD,
        minor: dollars * Money::SCALE,
    };
    let mut entries = Vec::with_capacity(periods * 4 + 1);

    // Opening capitalization + rehabilitation provision.
    entries.push(JournalEntry::build(
        "JE-0000-OPEN",
        "2026-00",
        site_id,
        vec![
            PostingLine {
                account: MiningAccount::Cash,
                side: EntrySide::Debit,
                amount: usd(250_000_000),
                memo: "Opening operating cash".to_string(),
            },
            PostingLine {
                account: MiningAccount::DeferredStrippingAsset,
                side: EntrySide::Debit,
                amount: usd(45_000_000),
                memo: "Opening deferred stripping".to_string(),
            },
            PostingLine {
                account: MiningAccount::RehabilitationProvision,
                side: EntrySide::Credit,
                amount: usd(35_000_000),
                memo: "ARO closure provision".to_string(),
            },
            PostingLine {
                account: MiningAccount::RetainedEquity,
                side: EntrySide::Credit,
                amount: usd(260_000_000),
                memo: "Opening retained equity".to_string(),
            },
        ],
    )?);

    for p in 1..=periods {
        let period = format!("2026-{p:02}");
        let mine_cost = 18_000_000 + (p as i128) * 150_000;
        let strip_cap = 3_500_000;
        let mill_cost = 12_500_000 + (p as i128) * 80_000;
        let da_cost = 4_200_000;
        let rev = 49_500_000 + (p as i128) * 300_000;
        let royalty = rev / 20; // exact 5% NSR

        // 1. Mining & deferred stripping -> Ore stockpile + cash out.
        entries.push(JournalEntry::build(
            format!("JE-{p:02}-MINE"),
            &period,
            site_id,
            vec![
                PostingLine {
                    account: MiningAccount::OreStockpileInventory,
                    side: EntrySide::Debit,
                    amount: usd(mine_cost),
                    memo: "Mined ore placed on ROM".to_string(),
                },
                PostingLine {
                    account: MiningAccount::DeferredStrippingAsset,
                    side: EntrySide::Debit,
                    amount: usd(strip_cap),
                    memo: "Capitalized waste stripping".to_string(),
                },
                PostingLine {
                    account: MiningAccount::Cash,
                    side: EntrySide::Credit,
                    amount: usd(mine_cost + strip_cap),
                    memo: "Mining contractor & fuel disbursement".to_string(),
                },
            ],
        )?);

        // 2. Milling & amortization -> Refined bullion inventory.
        entries.push(JournalEntry::build(
            format!("JE-{p:02}-MILL"),
            &period,
            site_id,
            vec![
                PostingLine {
                    account: MiningAccount::RefinedBullionInventory,
                    side: EntrySide::Debit,
                    amount: usd(mine_cost + mill_cost + da_cost),
                    memo: "Dore poured from ROM + mill".to_string(),
                },
                PostingLine {
                    account: MiningAccount::OreStockpileInventory,
                    side: EntrySide::Credit,
                    amount: usd(mine_cost),
                    memo: "ROM ore drawn to mill".to_string(),
                },
                PostingLine {
                    account: MiningAccount::Cash,
                    side: EntrySide::Credit,
                    amount: usd(mill_cost),
                    memo: "Processing reagents & power".to_string(),
                },
                PostingLine {
                    account: MiningAccount::DeferredStrippingAsset,
                    side: EntrySide::Credit,
                    amount: usd(da_cost),
                    memo: "Stripping amortization".to_string(),
                },
            ],
        )?);

        // 3. Bullion sale + COGS recognition + 5% royalty accrual.
        entries.push(JournalEntry::build(
            format!("JE-{p:02}-SALE"),
            &period,
            site_id,
            vec![
                PostingLine {
                    account: MiningAccount::Cash,
                    side: EntrySide::Debit,
                    amount: usd(rev),
                    memo: "Refinery outturn settlement".to_string(),
                },
                PostingLine {
                    account: MiningAccount::DoreSalesRevenue,
                    side: EntrySide::Credit,
                    amount: usd(rev),
                    memo: "Gold dore revenue".to_string(),
                },
                PostingLine {
                    account: MiningAccount::MiningOperatingExpense,
                    side: EntrySide::Debit,
                    amount: usd(mine_cost),
                    memo: "COGS mining component".to_string(),
                },
                PostingLine {
                    account: MiningAccount::ProcessingExpense,
                    side: EntrySide::Debit,
                    amount: usd(mill_cost),
                    memo: "COGS processing component".to_string(),
                },
                PostingLine {
                    account: MiningAccount::DepreciationExpense,
                    side: EntrySide::Debit,
                    amount: usd(da_cost),
                    memo: "COGS D&A component".to_string(),
                },
                PostingLine {
                    account: MiningAccount::RefinedBullionInventory,
                    side: EntrySide::Credit,
                    amount: usd(mine_cost + mill_cost + da_cost),
                    memo: "Bullion inventory relieved on sale".to_string(),
                },
                PostingLine {
                    account: MiningAccount::RoyaltyExpense,
                    side: EntrySide::Debit,
                    amount: usd(royalty),
                    memo: "5% NSR royalty expense".to_string(),
                },
                PostingLine {
                    account: MiningAccount::RoyaltyPayable,
                    side: EntrySide::Credit,
                    amount: usd(royalty),
                    memo: "5% NSR royalty payable".to_string(),
                },
            ],
        )?);
    }

    LocalAccountingBook::build(book_id, Currency::USD, entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_synthetic_fa_compatible_book_exact_double_entry_and_merkle_folding() {
        let book1 = generate_synthetic_mining_book("BOOK-SYNTH-AU", "SITE-SYNTH-01", 12)
            .expect("generate 12-month book");
        let book2 = generate_synthetic_mining_book("BOOK-SYNTH-AU", "SITE-SYNTH-01", 12)
            .expect("regenerate 12-month book");

        assert_eq!(book1.entries.len(), 1 + 12 * 3);
        assert_eq!(book1.total_debits, book1.total_credits);
        assert_eq!(book1.account_balances.values().copied().sum::<i128>(), 0);
        assert_eq!(book1.book_root_digest, book2.book_root_digest);

        // Opening + 12 months relieved ore and bullion inventory close cleanly to zero.
        assert_eq!(
            book1
                .account_balances
                .get(&MiningAccount::OreStockpileInventory)
                .copied(),
            Some(0)
        );
        assert_eq!(
            book1
                .account_balances
                .get(&MiningAccount::RefinedBullionInventory)
                .copied(),
            Some(0)
        );

        // Unbalanced entry must be refused with FMINE-REF-101 (never plugged).
        let bad = JournalEntry::build(
            "JE-BAD",
            "2026-01",
            "SITE-SYNTH-01",
            vec![
                PostingLine {
                    account: MiningAccount::Cash,
                    side: EntrySide::Debit,
                    amount: Money {
                        ccy: Currency::USD,
                        minor: 100 * Money::SCALE,
                    },
                    memo: "dr".to_string(),
                },
                PostingLine {
                    account: MiningAccount::DoreSalesRevenue,
                    side: EntrySide::Credit,
                    amount: Money {
                        ccy: Currency::USD,
                        minor: 99 * Money::SCALE,
                    },
                    memo: "cr short $1".to_string(),
                },
            ],
        )
        .unwrap_err();
        assert_eq!(bad.code, "FMINE-REF-101");
    }
}
