//! The eight drillhole record types of plan §12.1, their checks and their canonical bytes (`BD-163`).
//! Determinism: BitwisePortable (integer, byte and comparison operations only).
//!
//! - **Records.** [`Collar`], [`Survey`], [`Interval`], [`Sample`], [`Dispatch`], [`AssayResult`],
//!   [`QcInsertion`] and [`LabCertificate`], each named by its own id. [`DrillRecord`] holds any one of them.
//! - **Checks.** [`DrillRecord::validate`] refuses a record that is not physically or formally possible on its own:
//!   an interval whose `to` is not past its `from`, a dip outside ±90°, an assay mass fraction outside [0, 1]
//!   (Porgera's database held S > 100%, plan §12.1), a "no survey" station that carries an orientation. Checks
//!   between records (a survey of a hole with no collar) are the commit-time law's, in [`crate::store`].
//! - **Bytes.** [`encode_version`] writes one ledger version: an `FMNE` frame tagged `DRLR`, version 1, holding the
//!   record and, for a correction, its reason. [`decode_version`] reads it back and refuses a frame whose bytes are
//!   not the ones [`encode_version`] would write for what it holds, so every version has exactly one encoding.
//!   Floats are stored as IEEE-754 bits; `-0.0` is written as `0.0`; a non-finite float is refused.

use std::fmt;

use fmine_core::codec::{Decoder, Encoder};
use fmine_core::hash::Digest32;
use fmine_evidence::{Refusal, RefusalCode};

/// Frame tag of one record version.
pub const RECORD_TAG: [u8; 4] = *b"DRLR";
/// Frame version written and read.
pub const RECORD_VERSION: u16 = 1;
/// Longest record id, in bytes.
pub const MAX_ID_BYTES: usize = 128;
/// Longest free-text field (lithology, lab name, correction reason), in bytes.
pub const MAX_TEXT_BYTES: usize = 4096;
/// Most ids one record may list (a sample's intervals, a dispatch's samples) or custody events it may hold.
pub const MAX_LIST_ITEMS: usize = 4096;

/// A record that cannot be written or read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DrillError {
    /// A field outside what the record allows (`FMINE-REF-109`).
    Invalid {
        /// The record kind and field, for example `interval.to_mm`.
        field: String,
        /// What is wrong with it.
        detail: String,
    },
    /// Planned and actual collar positions in different CRSs (`FMINE-REF-106`).
    CrsMismatch {
        /// The hole.
        hole_id: String,
        /// The planned position's CRS.
        planned: String,
        /// The actual position's CRS.
        actual: String,
    },
    /// Bytes that are not one canonical record version (`FMINE-REF-505`).
    Corrupt {
        /// What the decoder found.
        detail: String,
    },
    /// Inputs whose support compositing cannot take (`FMINE-REF-102`).
    SupportMismatch {
        /// What is wrong.
        detail: String,
    },
    /// A non-additive quantity offered for compositing (`FMINE-REF-103`).
    NonAdditive {
        /// What is wrong.
        detail: String,
    },
    /// A composite set that does not conserve its inputs (`FMINE-REF-101`).
    Conservation {
        /// The totals that disagree.
        detail: String,
    },
}

impl DrillError {
    pub(crate) fn invalid(field: &str, detail: impl Into<String>) -> Self {
        Self::Invalid {
            field: field.to_string(),
            detail: detail.into(),
        }
    }

    fn corrupt(detail: impl Into<String>) -> Self {
        Self::Corrupt {
            detail: detail.into(),
        }
    }

    /// The refusal this error stands for.
    #[must_use]
    pub fn to_refusal(&self) -> Refusal {
        let code = match self {
            Self::Invalid { .. } => RefusalCode::OutsidePhysicalDomain109,
            Self::CrsMismatch { .. } => RefusalCode::CrsUnregisteredOrIncompatible106,
            Self::Corrupt { .. } => RefusalCode::DigestMismatch505,
            Self::SupportMismatch { .. } => RefusalCode::SupportMismatch102,
            Self::NonAdditive { .. } => RefusalCode::NonAdditiveAverage103,
            Self::Conservation { .. } => RefusalCode::Conservation101,
        };
        Refusal::new(code, self.to_string())
    }
}

impl fmt::Display for DrillError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid { field, detail } => write!(f, "{field}: {detail}"),
            Self::CrsMismatch {
                hole_id,
                planned,
                actual,
            } => write!(
                f,
                "collar '{hole_id}': planned position in {planned}, actual in {actual}; one CRS is required"
            ),
            Self::Corrupt { detail } => write!(f, "not a canonical drill record version: {detail}"),
            Self::SupportMismatch { detail } => write!(f, "support: {detail}"),
            Self::NonAdditive { detail } => write!(f, "additivity: {detail}"),
            Self::Conservation { detail } => write!(f, "conservation: {detail}"),
        }
    }
}

impl std::error::Error for DrillError {}

/// Declares a field-less enum with a one-byte tag and a stable lower-case code.
macro_rules! tagged_enum {
    ($(#[$meta:meta])* $name:ident { $($(#[$vmeta:meta])* $variant:ident = $tag:literal, $code:literal;)+ }) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum $name {
            $($(#[$vmeta])* $variant,)+
        }

        impl $name {
            /// Every variant, in tag order.
            pub const ALL: &'static [Self] = &[$(Self::$variant,)+];

            /// The stable code, as it appears in keys and reports.
            #[must_use]
            pub const fn code(self) -> &'static str {
                match self {
                    $(Self::$variant => $code,)+
                }
            }

            const fn tag(self) -> u8 {
                match self {
                    $(Self::$variant => $tag,)+
                }
            }

            fn from_tag(tag: u8) -> Result<Self, DrillError> {
                match tag {
                    $($tag => Ok(Self::$variant),)+
                    other => Err(DrillError::corrupt(format!(
                        "{} tag {other} is not defined",
                        stringify!($name)
                    ))),
                }
            }
        }
    };
}

tagged_enum! {
    /// The record families of plan §12.1 that this crate writes.
    RecordKind {
        /// [`Collar`].
        Collar = 1, "collar";
        /// [`Survey`].
        Survey = 2, "survey";
        /// [`Interval`].
        Interval = 3, "interval";
        /// [`Sample`].
        Sample = 4, "sample";
        /// [`Dispatch`].
        Dispatch = 5, "dispatch";
        /// [`AssayResult`].
        AssayResult = 6, "result";
        /// [`QcInsertion`].
        QcInsertion = 7, "qc";
        /// [`LabCertificate`].
        LabCertificate = 8, "certificate";
        /// [`SampleLoss`].
        SampleLoss = 9, "loss";
    }
}

impl RecordKind {
    /// The kind whose [`RecordKind::code`] is `code`.
    #[must_use]
    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|k| k.code() == code)
    }
}

tagged_enum! {
    /// Drilling technique (plan §12.1 `Collar`).
    HoleType {
        /// Diamond core (DD).
        Diamond = 1, "diamond";
        /// Reverse circulation (RC).
        ReverseCirculation = 2, "rc";
        /// Aircore.
        Aircore = 3, "aircore";
        /// Rotary air blast (RAB).
        Rab = 4, "rab";
        /// Sonic.
        Sonic = 5, "sonic";
        /// Production blasthole.
        Blasthole = 6, "blasthole";
    }
}

tagged_enum! {
    /// How a collar position was obtained.
    PositionMethod {
        /// The design (planned) position.
        Design = 1, "design";
        /// Differential or RTK GNSS.
        DifferentialGnss = 2, "dgnss";
        /// Total station.
        TotalStation = 3, "total_station";
        /// Hand-held GNSS.
        HandheldGnss = 4, "handheld_gnss";
        /// Not recorded; the position's sigma still states its accuracy.
        Unknown = 5, "unknown";
    }
}

tagged_enum! {
    /// Downhole survey instrument (plan §12.1 `Survey`). `None` is the state "this hole has no survey".
    SurveyInstrument {
        /// Gyroscope.
        Gyro = 1, "gyro";
        /// Magnetic multishot.
        MagneticMultishot = 2, "magnetic_multishot";
        /// Magnetic single shot.
        MagneticSingleShot = 3, "magnetic_single_shot";
        /// No survey. Never an assumed straight hole.
        None = 4, "none";
    }
}

tagged_enum! {
    /// Direction of the survey run.
    RunDirection {
        /// Surveyed going in.
        In = 1, "in";
        /// Surveyed coming out.
        Out = 2, "out";
    }
}

tagged_enum! {
    /// How core recovery was measured (plan §12.1 `Interval`).
    RecoveryMethod {
        /// Recovered length over drilled length.
        Length = 1, "length";
        /// Mass.
        Mass = 2, "mass";
        /// Volume.
        Volume = 3, "volume";
        /// Not recorded.
        Unknown = 4, "unknown";
    }
}

tagged_enum! {
    /// Sample type (plan §12.1 `Sample`).
    SampleType {
        /// Whole core.
        WholeCore = 1, "whole_core";
        /// Half core.
        HalfCore = 2, "half_core";
        /// Quarter core.
        QuarterCore = 3, "quarter_core";
        /// Reverse-circulation split.
        RcSplit = 4, "rc_split";
        /// Blasthole cone sample.
        BlastholeCone = 5, "blasthole_cone";
        /// Full blasthole sample.
        BlastholeFull = 6, "blasthole_full";
        /// Inserted QC material (a CRM or a blank), taken from no interval.
        QcMaterial = 7, "qc_material";
    }
}

tagged_enum! {
    /// Assay method. Each one measures its own measurand, so results of different methods are never merged
    /// (plan §12.1 `AssayResult`).
    AssayMethod {
        /// Fire assay, AAS finish.
        FireAssayAas = 1, "fa_aas";
        /// Fire assay, gravimetric finish.
        FireAssayGravimetric = 2, "fa_grav";
        /// Screen fire assay.
        ScreenFireAssay = 3, "screen_fa";
        /// PhotonAssay.
        PhotonAssay = 4, "photon";
        /// LeachWELL.
        LeachWell = 5, "leachwell";
        /// Bulk leach extractable gold.
        Bleg = 6, "bleg";
        /// Four-acid digest, ICP-MS.
        FourAcidIcpMs = 7, "4a_icpms";
        /// Four-acid digest, ICP-OES.
        FourAcidIcpOes = 8, "4a_icpoes";
        /// Aqua regia digest.
        AquaRegia = 9, "aqua_regia";
        /// Fusion XRF.
        FusionXrf = 10, "fusion_xrf";
        /// LECO sulphur and carbon.
        LecoSc = 11, "leco_sc";
        /// Sequential copper leach, total copper.
        SequentialCuTotal = 12, "seq_cu_t";
        /// Sequential copper leach, acid-soluble copper.
        SequentialCuAcidSoluble = 13, "seq_cu_as";
        /// Sequential copper leach, cyanide-soluble copper.
        SequentialCuCyanideSoluble = 14, "seq_cu_cn";
    }
}

tagged_enum! {
    /// QA/QC regime of the campaign a result belongs to (plan §12.1 `AssayResult`).
    QaqcRegime {
        /// No QA/QC.
        None = 1, "none";
        /// Partial QA/QC.
        Partial = 2, "partial";
        /// Current QA/QC.
        Current = 3, "current";
    }
}

tagged_enum! {
    /// Preparation stage a blank is inserted at.
    BlankStage {
        /// Coarse blank.
        Coarse = 1, "coarse";
        /// Pulp blank.
        Pulp = 2, "pulp";
    }
}

tagged_enum! {
    /// Stage a duplicate is split at.
    DuplicateStage {
        /// Field duplicate.
        Field = 1, "field";
        /// Coarse-reject duplicate.
        CoarseReject = 2, "coarse_reject";
        /// Pulp duplicate.
        Pulp = 3, "pulp";
    }
}

/// A CRS-registered position with its accuracy (plan §12.1: "position accuracy is an error model, not a comment").
#[derive(Clone, Debug, PartialEq)]
pub struct Position {
    /// Registered CRS code, for example `EPSG:32735`.
    pub crs: String,
    /// Easting, m.
    pub easting_m: f64,
    /// Northing, m.
    pub northing_m: f64,
    /// Elevation, m.
    pub elevation_m: f64,
    /// How the position was obtained.
    pub method: PositionMethod,
    /// One-sigma horizontal accuracy, m.
    pub sigma_horizontal_m: f64,
    /// One-sigma vertical accuracy, m.
    pub sigma_vertical_m: f64,
}

/// `Collar` (plan §12.1).
#[derive(Clone, Debug, PartialEq)]
pub struct Collar {
    /// Hole id; the record's id.
    pub hole_id: String,
    /// Drilling technique.
    pub hole_type: HoleType,
    /// Drilling program.
    pub program: String,
    /// Planned position, if recorded.
    pub planned: Option<Position>,
    /// Surveyed actual position, if recorded.
    pub actual: Option<Position>,
}

/// A measured survey orientation. Dip is from horizontal, negative downward (−90° is vertically down).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Orientation {
    /// Azimuth, degrees in [0, 360).
    pub azimuth_deg: f64,
    /// Dip, degrees in [−90, 90].
    pub dip_deg: f64,
    /// One-sigma azimuth accuracy, degrees.
    pub sigma_azimuth_deg: f64,
    /// One-sigma dip accuracy, degrees.
    pub sigma_dip_deg: f64,
}

/// `Survey` station (plan §12.1).
#[derive(Clone, Debug, PartialEq)]
pub struct Survey {
    /// Survey station id; the record's id.
    pub survey_id: String,
    /// The surveyed hole.
    pub hole_id: String,
    /// Depth along the hole, mm.
    pub depth_mm: u64,
    /// Instrument; [`SurveyInstrument::None`] declares that the hole has no survey.
    pub instrument: SurveyInstrument,
    /// The measured orientation. Present exactly when an instrument was used.
    pub orientation: Option<Orientation>,
    /// Magnetic interference flagged at this station.
    pub magnetic_interference: bool,
    /// Run direction.
    pub run: RunDirection,
}

impl Survey {
    /// A magnetic reading with interference flagged: `Suspect` until a gyro check exists (plan §12.1, E380, E381).
    #[must_use]
    pub fn needs_gyro_check(&self) -> bool {
        self.magnetic_interference
            && matches!(
                self.instrument,
                SurveyInstrument::MagneticMultishot | SurveyInstrument::MagneticSingleShot
            )
    }
}

/// Core recovery with the method that measured it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Recovery {
    /// Recovery, percent (finite and at least 0; above 100 is possible and kept).
    pub percent: f64,
    /// How it was measured.
    pub method: RecoveryMethod,
}

/// Alteration type and intensity as logged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Alteration {
    /// Alteration type.
    pub kind: String,
    /// Intensity, as the logging scheme names it.
    pub intensity: String,
}

/// Who or what logged an interval. Each logger is its own evidence source (plan §12.1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Logger {
    /// A geologist.
    Human {
        /// Logger id.
        logger_id: String,
    },
    /// A core scanner.
    Scanner {
        /// Instrument id.
        instrument_id: String,
    },
    /// A model; its logs are `ModelledUncalibrated` until scored against human relogs.
    Model {
        /// Model name.
        name: String,
        /// Model version.
        version: String,
    },
}

/// Logged `Interval` (plan §12.1).
#[derive(Clone, Debug, PartialEq)]
pub struct Interval {
    /// Interval id; the record's id.
    pub interval_id: String,
    /// The hole.
    pub hole_id: String,
    /// From depth, mm.
    pub from_mm: u64,
    /// To depth, mm; greater than `from_mm`.
    pub to_mm: u64,
    /// Core recovery, if measured.
    pub recovery: Option<Recovery>,
    /// Lithology code.
    pub lithology: String,
    /// Alteration, if logged.
    pub alteration: Option<Alteration>,
    /// Mineralization, as logged.
    pub mineralization: String,
    /// Weathering, as logged.
    pub weathering: String,
    /// Who or what logged it.
    pub logger: Logger,
}

/// One custody hand-over.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CustodyEvent {
    /// When, ns since the Unix epoch, UTC.
    pub at_ns: i64,
    /// Who took custody.
    pub holder: String,
}

/// `Sample` (plan §12.1). Its support is first-class: the intervals it came from and its mass.
#[derive(Clone, Debug, PartialEq)]
pub struct Sample {
    /// Sample id; the record's id.
    pub sample_id: String,
    /// The intervals it was taken from, ascending and without repeats; none for [`SampleType::QcMaterial`].
    pub interval_ids: Vec<String>,
    /// Mass, g, if weighed.
    pub mass_g: Option<f64>,
    /// Sample type.
    pub sample_type: SampleType,
    /// Splitter used, as recorded.
    pub splitter: String,
    /// Chain of custody, in strictly increasing time.
    pub custody: Vec<CustodyEvent>,
}

/// `Dispatch` batch of samples to a lab.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dispatch {
    /// Dispatch id; the record's id.
    pub dispatch_id: String,
    /// The receiving lab.
    pub lab: String,
    /// The samples sent, ascending and without repeats.
    pub sample_ids: Vec<String>,
    /// When it left, ns since the Unix epoch, UTC.
    pub dispatched_at_ns: i64,
}

/// An assay value as a mass fraction in [0, 1], or a censored bound. A censored value is never replaced by a
/// number such as DL/2 (guard G-CENS, plan §12.2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AssayValue {
    /// A measured mass fraction.
    Measured {
        /// Mass fraction in [0, 1].
        mass_fraction: f64,
    },
    /// Below the detection limit.
    BelowDetection {
        /// Detection limit as a mass fraction in (0, 1].
        limit: f64,
    },
    /// Above the upper limit.
    AboveUpper {
        /// Upper limit as a mass fraction in (0, 1].
        limit: f64,
    },
}

/// Where a result's certificate is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CertificateRef {
    /// A [`LabCertificate`] record.
    Certificate {
        /// Its id.
        certificate_id: String,
    },
    /// Delivered straight from the lab's LIMS with no certificate (plan §12.1 `LabCertificate`).
    LimsDirectNoCertificate,
}

/// `AssayResult` (plan §12.1).
#[derive(Clone, Debug, PartialEq)]
pub struct AssayResult {
    /// Result id; the record's id.
    pub result_id: String,
    /// The assayed sample.
    pub sample_id: String,
    /// The dispatch that sent it; it lists the sample and names the same lab.
    pub dispatch_id: String,
    /// Analyte, for example `Au`.
    pub analyte: String,
    /// Method; it defines the measurand.
    pub method: AssayMethod,
    /// The value.
    pub value: AssayValue,
    /// The lab that reported it.
    pub lab: String,
    /// The lab's batch id.
    pub lab_batch: String,
    /// Its certificate.
    pub certificate: CertificateRef,
    /// QA/QC regime of the campaign.
    pub qaqc_regime: QaqcRegime,
    /// When the result was received from the lab, ns since the Unix epoch, UTC; not before its dispatch.
    pub received_at_ns: i64,
}

/// A dispatched sample declared lost with the QA/QC finding that says so (`OB-LAB`, plan §12.4): it closes the
/// sample's lab obligation without a result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SampleLoss {
    /// Declaration id; the record's id.
    pub loss_id: String,
    /// The dispatch that sent the sample; it lists the sample.
    pub dispatch_id: String,
    /// The lost sample.
    pub sample_id: String,
    /// When the loss was declared, ns since the Unix epoch, UTC; not before the dispatch.
    pub declared_at_ns: i64,
    /// The QA/QC finding.
    pub finding: String,
}

/// What a QC insertion is.
#[derive(Clone, Debug, PartialEq)]
pub enum QcKind {
    /// Certified reference material.
    Crm {
        /// CRM id.
        crm_id: String,
        /// Certified mean, mass fraction in [0, 1].
        certified_mass_fraction: f64,
        /// Certified between-lab sigma, mass fraction (σ_C).
        sigma_c: f64,
        /// Certified within-lab sigma, mass fraction (σ_L).
        sigma_l: f64,
        /// Source of the certificate.
        certificate_source: String,
    },
    /// Blank.
    Blank {
        /// Stage it is inserted at.
        stage: BlankStage,
    },
    /// Duplicate of another sample.
    Duplicate {
        /// Split stage.
        stage: DuplicateStage,
        /// The sample it duplicates.
        original_sample_id: String,
    },
    /// Umpire assay of another sample at a second lab.
    Umpire {
        /// The sample it checks.
        original_sample_id: String,
        /// The umpire lab.
        umpire_lab: String,
    },
}

/// `QaqcInsertion` (plan §12.1): the QC role of one sample.
#[derive(Clone, Debug, PartialEq)]
pub struct QcInsertion {
    /// Insertion id; the record's id.
    pub insertion_id: String,
    /// The sample slot that carries it.
    pub sample_id: String,
    /// What it is.
    pub kind: QcKind,
}

/// `LabCertificate` (plan §12.1): the signed original's digest and the `FMT-CAS-1` root of its copy in CAS
/// (`BD-171`). Reading the root back must give bytes with that digest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LabCertificate {
    /// Certificate id; the record's id.
    pub certificate_id: String,
    /// Issuing lab.
    pub lab: String,
    /// SHA-256 of the original bytes.
    pub original_sha256: Digest32,
    /// Fanout-16 root of the original's copy in CAS.
    pub cas_root: Digest32,
    /// Media type of the original, for example `application/pdf`.
    pub media_type: String,
    /// When it was issued, ns since the Unix epoch, UTC.
    pub issued_at_ns: i64,
}

/// Any one drillhole record.
#[derive(Clone, Debug, PartialEq)]
pub enum DrillRecord {
    /// [`Collar`].
    Collar(Collar),
    /// [`Survey`].
    Survey(Survey),
    /// [`Interval`].
    Interval(Interval),
    /// [`Sample`].
    Sample(Sample),
    /// [`Dispatch`].
    Dispatch(Dispatch),
    /// [`AssayResult`].
    AssayResult(AssayResult),
    /// [`QcInsertion`].
    QcInsertion(QcInsertion),
    /// [`LabCertificate`].
    LabCertificate(LabCertificate),
    /// [`SampleLoss`].
    SampleLoss(SampleLoss),
}

impl DrillRecord {
    /// The record's kind.
    #[must_use]
    pub const fn kind(&self) -> RecordKind {
        match self {
            Self::Collar(_) => RecordKind::Collar,
            Self::Survey(_) => RecordKind::Survey,
            Self::Interval(_) => RecordKind::Interval,
            Self::Sample(_) => RecordKind::Sample,
            Self::Dispatch(_) => RecordKind::Dispatch,
            Self::AssayResult(_) => RecordKind::AssayResult,
            Self::QcInsertion(_) => RecordKind::QcInsertion,
            Self::LabCertificate(_) => RecordKind::LabCertificate,
            Self::SampleLoss(_) => RecordKind::SampleLoss,
        }
    }

    /// The record's own id.
    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            Self::Collar(r) => &r.hole_id,
            Self::Survey(r) => &r.survey_id,
            Self::Interval(r) => &r.interval_id,
            Self::Sample(r) => &r.sample_id,
            Self::Dispatch(r) => &r.dispatch_id,
            Self::AssayResult(r) => &r.result_id,
            Self::QcInsertion(r) => &r.insertion_id,
            Self::LabCertificate(r) => &r.certificate_id,
            Self::SampleLoss(r) => &r.loss_id,
        }
    }

    /// Records this one names, which must exist when it is written.
    #[must_use]
    pub fn references(&self) -> Vec<(RecordKind, &str)> {
        match self {
            Self::Collar(_) | Self::LabCertificate(_) => Vec::new(),
            Self::Survey(r) => vec![(RecordKind::Collar, r.hole_id.as_str())],
            Self::Interval(r) => vec![(RecordKind::Collar, r.hole_id.as_str())],
            Self::Sample(r) => r
                .interval_ids
                .iter()
                .map(|id| (RecordKind::Interval, id.as_str()))
                .collect(),
            Self::Dispatch(r) => r
                .sample_ids
                .iter()
                .map(|id| (RecordKind::Sample, id.as_str()))
                .collect(),
            Self::AssayResult(r) => {
                let mut refs = vec![
                    (RecordKind::Sample, r.sample_id.as_str()),
                    (RecordKind::Dispatch, r.dispatch_id.as_str()),
                ];
                if let CertificateRef::Certificate { certificate_id } = &r.certificate {
                    refs.push((RecordKind::LabCertificate, certificate_id.as_str()));
                }
                refs
            }
            Self::SampleLoss(r) => vec![
                (RecordKind::Sample, r.sample_id.as_str()),
                (RecordKind::Dispatch, r.dispatch_id.as_str()),
            ],
            Self::QcInsertion(r) => {
                let mut refs = vec![(RecordKind::Sample, r.sample_id.as_str())];
                if let QcKind::Duplicate {
                    original_sample_id, ..
                }
                | QcKind::Umpire {
                    original_sample_id, ..
                } = &r.kind
                {
                    refs.push((RecordKind::Sample, original_sample_id.as_str()));
                }
                refs
            }
        }
    }

    /// Refuse a record that cannot be true on its own.
    ///
    /// # Errors
    /// [`DrillError::Invalid`] naming the field, or [`DrillError::CrsMismatch`] for a collar.
    pub fn validate(&self) -> Result<(), DrillError> {
        match self {
            Self::Collar(r) => validate_collar(r),
            Self::Survey(r) => validate_survey(r),
            Self::Interval(r) => validate_interval(r),
            Self::Sample(r) => validate_sample(r),
            Self::Dispatch(r) => validate_dispatch(r),
            Self::AssayResult(r) => validate_result(r),
            Self::QcInsertion(r) => validate_qc(r),
            Self::LabCertificate(r) => {
                check_id("certificate.certificate_id", &r.certificate_id)?;
                check_name("certificate.lab", &r.lab)?;
                check_name("certificate.media_type", &r.media_type)
            }
            Self::SampleLoss(r) => {
                check_id("loss.loss_id", &r.loss_id)?;
                check_id("loss.dispatch_id", &r.dispatch_id)?;
                check_id("loss.sample_id", &r.sample_id)?;
                check_name("loss.finding", &r.finding)
            }
        }
    }
}

/// An id: 1 to [`MAX_ID_BYTES`] bytes of printable ASCII other than `/`, so that it can end a ledger key.
fn check_id(field: &str, id: &str) -> Result<(), DrillError> {
    if id.is_empty() || id.len() > MAX_ID_BYTES {
        return Err(DrillError::invalid(
            field,
            format!("an id is 1 to {MAX_ID_BYTES} bytes, got {}", id.len()),
        ));
    }
    if let Some(c) = id.chars().find(|&c| !c.is_ascii_graphic() || c == '/') {
        return Err(DrillError::invalid(
            field,
            format!("an id is printable ASCII without '/', found {c:?}"),
        ));
    }
    Ok(())
}

/// Free text of at most [`MAX_TEXT_BYTES`] bytes; may be empty.
fn check_text(field: &str, text: &str) -> Result<(), DrillError> {
    if text.len() > MAX_TEXT_BYTES {
        return Err(DrillError::invalid(
            field,
            format!("at most {MAX_TEXT_BYTES} bytes, got {}", text.len()),
        ));
    }
    Ok(())
}

/// Free text that must say something.
fn check_name(field: &str, text: &str) -> Result<(), DrillError> {
    if text.trim().is_empty() {
        return Err(DrillError::invalid(field, "must not be empty"));
    }
    check_text(field, text)
}

fn check_finite(field: &str, v: f64) -> Result<(), DrillError> {
    if v.is_finite() {
        Ok(())
    } else {
        Err(DrillError::invalid(field, format!("{v} is not finite")))
    }
}

fn check_non_negative(field: &str, v: f64) -> Result<(), DrillError> {
    check_finite(field, v)?;
    if v < 0.0 {
        return Err(DrillError::invalid(field, format!("{v} is negative")));
    }
    Ok(())
}

fn check_fraction(field: &str, v: f64, zero_allowed: bool) -> Result<(), DrillError> {
    check_finite(field, v)?;
    let low_ok = if zero_allowed { v >= 0.0 } else { v > 0.0 };
    if !low_ok || v > 1.0 {
        let range = if zero_allowed { "[0, 1]" } else { "(0, 1]" };
        return Err(DrillError::invalid(
            field,
            format!("mass fraction {v} is outside {range}"),
        ));
    }
    Ok(())
}

fn check_ids(field: &str, ids: &[String]) -> Result<(), DrillError> {
    if ids.is_empty() || ids.len() > MAX_LIST_ITEMS {
        return Err(DrillError::invalid(
            field,
            format!("1 to {MAX_LIST_ITEMS} ids, got {}", ids.len()),
        ));
    }
    for id in ids {
        check_id(field, id)?;
    }
    if ids.windows(2).any(|w| w[0] >= w[1]) {
        return Err(DrillError::invalid(
            field,
            "ids must be ascending without repeats",
        ));
    }
    Ok(())
}

fn validate_position(field: &str, p: &Position) -> Result<(), DrillError> {
    check_name(&format!("{field}.crs"), &p.crs)?;
    check_finite(&format!("{field}.easting_m"), p.easting_m)?;
    check_finite(&format!("{field}.northing_m"), p.northing_m)?;
    check_finite(&format!("{field}.elevation_m"), p.elevation_m)?;
    check_non_negative(&format!("{field}.sigma_horizontal_m"), p.sigma_horizontal_m)?;
    check_non_negative(&format!("{field}.sigma_vertical_m"), p.sigma_vertical_m)
}

pub(crate) fn validate_collar(r: &Collar) -> Result<(), DrillError> {
    check_id("collar.hole_id", &r.hole_id)?;
    check_text("collar.program", &r.program)?;
    if let Some(p) = &r.planned {
        validate_position("collar.planned", p)?;
    }
    if let Some(p) = &r.actual {
        validate_position("collar.actual", p)?;
    }
    match (&r.planned, &r.actual) {
        (None, None) => Err(DrillError::invalid(
            "collar.planned",
            "a collar needs a planned or an actual position",
        )),
        (Some(p), Some(a)) if p.crs != a.crs => Err(DrillError::CrsMismatch {
            hole_id: r.hole_id.clone(),
            planned: p.crs.clone(),
            actual: a.crs.clone(),
        }),
        _ => Ok(()),
    }
}

pub(crate) fn validate_survey(r: &Survey) -> Result<(), DrillError> {
    check_id("survey.survey_id", &r.survey_id)?;
    check_id("survey.hole_id", &r.hole_id)?;
    match (r.instrument, &r.orientation) {
        (SurveyInstrument::None, Some(_)) => Err(DrillError::invalid(
            "survey.orientation",
            "a station with no instrument has no orientation",
        )),
        (SurveyInstrument::None, None) => Ok(()),
        (_, None) => Err(DrillError::invalid(
            "survey.orientation",
            "a surveyed station needs its orientation; record instrument none for an unsurveyed hole",
        )),
        (_, Some(o)) => {
            check_finite("survey.azimuth_deg", o.azimuth_deg)?;
            if !(0.0..360.0).contains(&o.azimuth_deg) {
                return Err(DrillError::invalid(
                    "survey.azimuth_deg",
                    format!("{} is outside [0, 360)", o.azimuth_deg),
                ));
            }
            check_finite("survey.dip_deg", o.dip_deg)?;
            if !(-90.0..=90.0).contains(&o.dip_deg) {
                return Err(DrillError::invalid(
                    "survey.dip_deg",
                    format!("{} is outside [-90, 90]", o.dip_deg),
                ));
            }
            check_non_negative("survey.sigma_azimuth_deg", o.sigma_azimuth_deg)?;
            check_non_negative("survey.sigma_dip_deg", o.sigma_dip_deg)
        }
    }
}

fn validate_logger(l: &Logger) -> Result<(), DrillError> {
    match l {
        Logger::Human { logger_id } => check_name("interval.logger.logger_id", logger_id),
        Logger::Scanner { instrument_id } => {
            check_name("interval.logger.instrument_id", instrument_id)
        }
        Logger::Model { name, version } => {
            check_name("interval.logger.name", name)?;
            check_name("interval.logger.version", version)
        }
    }
}

fn validate_interval(r: &Interval) -> Result<(), DrillError> {
    check_id("interval.interval_id", &r.interval_id)?;
    check_id("interval.hole_id", &r.hole_id)?;
    if r.to_mm <= r.from_mm {
        return Err(DrillError::invalid(
            "interval.to_mm",
            format!("to {} mm is not past from {} mm", r.to_mm, r.from_mm),
        ));
    }
    if let Some(rec) = &r.recovery {
        check_non_negative("interval.recovery.percent", rec.percent)?;
    }
    check_text("interval.lithology", &r.lithology)?;
    if let Some(a) = &r.alteration {
        check_name("interval.alteration.kind", &a.kind)?;
        check_text("interval.alteration.intensity", &a.intensity)?;
    }
    check_text("interval.mineralization", &r.mineralization)?;
    check_text("interval.weathering", &r.weathering)?;
    validate_logger(&r.logger)
}

fn validate_sample(r: &Sample) -> Result<(), DrillError> {
    check_id("sample.sample_id", &r.sample_id)?;
    if r.sample_type == SampleType::QcMaterial {
        if !r.interval_ids.is_empty() {
            return Err(DrillError::invalid(
                "sample.interval_ids",
                "inserted QC material is taken from no interval",
            ));
        }
    } else {
        check_ids("sample.interval_ids", &r.interval_ids)?;
    }
    if let Some(m) = r.mass_g {
        check_finite("sample.mass_g", m)?;
        if m <= 0.0 {
            return Err(DrillError::invalid(
                "sample.mass_g",
                format!("{m} g is not a mass"),
            ));
        }
    }
    check_text("sample.splitter", &r.splitter)?;
    if r.custody.len() > MAX_LIST_ITEMS {
        return Err(DrillError::invalid(
            "sample.custody",
            format!("at most {MAX_LIST_ITEMS} events"),
        ));
    }
    for e in &r.custody {
        check_name("sample.custody.holder", &e.holder)?;
    }
    if r.custody.windows(2).any(|w| w[0].at_ns >= w[1].at_ns) {
        return Err(DrillError::invalid(
            "sample.custody",
            "custody events must be in strictly increasing time",
        ));
    }
    Ok(())
}

fn validate_dispatch(r: &Dispatch) -> Result<(), DrillError> {
    check_id("dispatch.dispatch_id", &r.dispatch_id)?;
    check_name("dispatch.lab", &r.lab)?;
    check_ids("dispatch.sample_ids", &r.sample_ids)
}

fn validate_result(r: &AssayResult) -> Result<(), DrillError> {
    check_id("result.result_id", &r.result_id)?;
    check_id("result.sample_id", &r.sample_id)?;
    check_id("result.dispatch_id", &r.dispatch_id)?;
    check_name("result.analyte", &r.analyte)?;
    match r.value {
        AssayValue::Measured { mass_fraction } => {
            check_fraction("result.value", mass_fraction, true)?;
        }
        AssayValue::BelowDetection { limit } | AssayValue::AboveUpper { limit } => {
            check_fraction("result.value.limit", limit, false)?;
        }
    }
    check_name("result.lab", &r.lab)?;
    check_text("result.lab_batch", &r.lab_batch)?;
    if let CertificateRef::Certificate { certificate_id } = &r.certificate {
        check_id("result.certificate", certificate_id)?;
    }
    Ok(())
}

fn validate_qc(r: &QcInsertion) -> Result<(), DrillError> {
    check_id("qc.insertion_id", &r.insertion_id)?;
    check_id("qc.sample_id", &r.sample_id)?;
    match &r.kind {
        QcKind::Crm {
            crm_id,
            certified_mass_fraction,
            sigma_c,
            sigma_l,
            certificate_source,
        } => {
            check_name("qc.crm_id", crm_id)?;
            check_fraction("qc.certified_mass_fraction", *certified_mass_fraction, true)?;
            check_non_negative("qc.sigma_c", *sigma_c)?;
            check_non_negative("qc.sigma_l", *sigma_l)?;
            check_name("qc.certificate_source", certificate_source)
        }
        QcKind::Blank { .. } => Ok(()),
        QcKind::Duplicate {
            original_sample_id, ..
        }
        | QcKind::Umpire {
            original_sample_id, ..
        } => {
            check_id("qc.original_sample_id", original_sample_id)?;
            if *original_sample_id == r.sample_id {
                return Err(DrillError::invalid(
                    "qc.original_sample_id",
                    "a duplicate or umpire sample cannot check itself",
                ));
            }
            if let QcKind::Umpire { umpire_lab, .. } = &r.kind {
                check_name("qc.umpire_lab", umpire_lab)?;
            }
            Ok(())
        }
    }
}

/// One ledger version of a record, as read back.
#[derive(Clone, Debug, PartialEq)]
pub struct RecordVersion {
    /// The record.
    pub record: DrillRecord,
    /// Why it supersedes the version before it; `None` for the first version.
    pub correction: Option<String>,
}

fn put_f64(enc: &mut Encoder, v: f64) {
    // Validation has refused non-finite values; -0.0 and 0.0 get one encoding.
    let v = if v == 0.0 { 0.0 } else { v };
    enc.put_u64(v.to_bits());
}

fn put_opt<T: ?Sized>(enc: &mut Encoder, v: Option<&T>, write: impl FnOnce(&mut Encoder, &T)) {
    match v {
        None => enc.put_u8(0),
        Some(x) => {
            enc.put_u8(1);
            write(enc, x);
        }
    }
}

fn put_position(enc: &mut Encoder, p: &Position) {
    enc.put_str(&p.crs);
    put_f64(enc, p.easting_m);
    put_f64(enc, p.northing_m);
    put_f64(enc, p.elevation_m);
    enc.put_u8(p.method.tag());
    put_f64(enc, p.sigma_horizontal_m);
    put_f64(enc, p.sigma_vertical_m);
}

fn put_ids(enc: &mut Encoder, ids: &[String]) -> Result<(), DrillError> {
    enc.put_canonical_seq(ids, |e, id| e.put_str(id))
        .map_err(|e| DrillError::invalid("ids", e.to_string()))
}

fn put_record(enc: &mut Encoder, record: &DrillRecord) -> Result<(), DrillError> {
    enc.put_u8(record.kind().tag());
    match record {
        DrillRecord::Collar(r) => {
            enc.put_str(&r.hole_id);
            enc.put_u8(r.hole_type.tag());
            enc.put_str(&r.program);
            put_opt(enc, r.planned.as_ref(), put_position);
            put_opt(enc, r.actual.as_ref(), put_position);
        }
        DrillRecord::Survey(r) => {
            enc.put_str(&r.survey_id);
            enc.put_str(&r.hole_id);
            enc.put_u64(r.depth_mm);
            enc.put_u8(r.instrument.tag());
            put_opt(enc, r.orientation.as_ref(), |e, o| {
                put_f64(e, o.azimuth_deg);
                put_f64(e, o.dip_deg);
                put_f64(e, o.sigma_azimuth_deg);
                put_f64(e, o.sigma_dip_deg);
            });
            enc.put_u8(u8::from(r.magnetic_interference));
            enc.put_u8(r.run.tag());
        }
        DrillRecord::Interval(r) => {
            enc.put_str(&r.interval_id);
            enc.put_str(&r.hole_id);
            enc.put_u64(r.from_mm);
            enc.put_u64(r.to_mm);
            put_opt(enc, r.recovery.as_ref(), |e, rec| {
                put_f64(e, rec.percent);
                e.put_u8(rec.method.tag());
            });
            enc.put_str(&r.lithology);
            put_opt(enc, r.alteration.as_ref(), |e, a| {
                e.put_str(&a.kind);
                e.put_str(&a.intensity);
            });
            enc.put_str(&r.mineralization);
            enc.put_str(&r.weathering);
            match &r.logger {
                Logger::Human { logger_id } => {
                    enc.put_u8(1);
                    enc.put_str(logger_id);
                }
                Logger::Scanner { instrument_id } => {
                    enc.put_u8(2);
                    enc.put_str(instrument_id);
                }
                Logger::Model { name, version } => {
                    enc.put_u8(3);
                    enc.put_str(name);
                    enc.put_str(version);
                }
            }
        }
        DrillRecord::Sample(r) => {
            enc.put_str(&r.sample_id);
            put_ids(enc, &r.interval_ids)?;
            put_opt(enc, r.mass_g.as_ref(), |e, m| put_f64(e, *m));
            enc.put_u8(r.sample_type.tag());
            enc.put_str(&r.splitter);
            enc.put_canonical_seq(&r.custody, |e, c| {
                e.put_i64(c.at_ns);
                e.put_str(&c.holder);
            })
            .map_err(|e| DrillError::invalid("sample.custody", e.to_string()))?;
        }
        DrillRecord::Dispatch(r) => {
            enc.put_str(&r.dispatch_id);
            enc.put_str(&r.lab);
            put_ids(enc, &r.sample_ids)?;
            enc.put_i64(r.dispatched_at_ns);
        }
        DrillRecord::AssayResult(r) => {
            enc.put_str(&r.result_id);
            enc.put_str(&r.sample_id);
            enc.put_str(&r.dispatch_id);
            enc.put_str(&r.analyte);
            enc.put_u8(r.method.tag());
            let (tag, v) = match r.value {
                AssayValue::Measured { mass_fraction } => (1, mass_fraction),
                AssayValue::BelowDetection { limit } => (2, limit),
                AssayValue::AboveUpper { limit } => (3, limit),
            };
            enc.put_u8(tag);
            put_f64(enc, v);
            enc.put_str(&r.lab);
            enc.put_str(&r.lab_batch);
            match &r.certificate {
                CertificateRef::Certificate { certificate_id } => {
                    enc.put_u8(1);
                    enc.put_str(certificate_id);
                }
                CertificateRef::LimsDirectNoCertificate => enc.put_u8(2),
            }
            enc.put_u8(r.qaqc_regime.tag());
            enc.put_i64(r.received_at_ns);
        }
        DrillRecord::QcInsertion(r) => {
            enc.put_str(&r.insertion_id);
            enc.put_str(&r.sample_id);
            match &r.kind {
                QcKind::Crm {
                    crm_id,
                    certified_mass_fraction,
                    sigma_c,
                    sigma_l,
                    certificate_source,
                } => {
                    enc.put_u8(1);
                    enc.put_str(crm_id);
                    put_f64(enc, *certified_mass_fraction);
                    put_f64(enc, *sigma_c);
                    put_f64(enc, *sigma_l);
                    enc.put_str(certificate_source);
                }
                QcKind::Blank { stage } => {
                    enc.put_u8(2);
                    enc.put_u8(stage.tag());
                }
                QcKind::Duplicate {
                    stage,
                    original_sample_id,
                } => {
                    enc.put_u8(3);
                    enc.put_u8(stage.tag());
                    enc.put_str(original_sample_id);
                }
                QcKind::Umpire {
                    original_sample_id,
                    umpire_lab,
                } => {
                    enc.put_u8(4);
                    enc.put_str(original_sample_id);
                    enc.put_str(umpire_lab);
                }
            }
        }
        DrillRecord::LabCertificate(r) => {
            enc.put_str(&r.certificate_id);
            enc.put_str(&r.lab);
            enc.put_fixed(&r.original_sha256.0);
            enc.put_fixed(&r.cas_root.0);
            enc.put_str(&r.media_type);
            enc.put_i64(r.issued_at_ns);
        }
        DrillRecord::SampleLoss(r) => {
            enc.put_str(&r.loss_id);
            enc.put_str(&r.dispatch_id);
            enc.put_str(&r.sample_id);
            enc.put_i64(r.declared_at_ns);
            enc.put_str(&r.finding);
        }
    }
    Ok(())
}

/// Write one ledger version of `record`: the first version when `correction` is `None`, otherwise a correction
/// with its reason.
///
/// # Errors
/// [`DrillError::Invalid`] or [`DrillError::CrsMismatch`] from [`DrillRecord::validate`], and
/// [`DrillError::Invalid`] for a correction whose reason is empty or longer than [`MAX_TEXT_BYTES`].
pub fn encode_version(
    record: &DrillRecord,
    correction: Option<&str>,
) -> Result<Vec<u8>, DrillError> {
    record.validate()?;
    if let Some(reason) = correction {
        check_name("correction", reason)?;
    }
    let mut enc = Encoder::new_frame(RECORD_TAG, RECORD_VERSION);
    put_record(&mut enc, record)?;
    put_opt(&mut enc, correction, |e, reason| e.put_str(reason));
    Ok(enc.finish())
}

struct Reader<'a, 'b> {
    dec: &'b mut Decoder<'a>,
}

impl Reader<'_, '_> {
    fn u8(&mut self) -> Result<u8, DrillError> {
        self.dec
            .get_u8()
            .map_err(|e| DrillError::corrupt(e.to_string()))
    }

    fn u64(&mut self) -> Result<u64, DrillError> {
        self.dec
            .get_u64()
            .map_err(|e| DrillError::corrupt(e.to_string()))
    }

    fn i64(&mut self) -> Result<i64, DrillError> {
        self.dec
            .get_i64()
            .map_err(|e| DrillError::corrupt(e.to_string()))
    }

    fn f64(&mut self) -> Result<f64, DrillError> {
        Ok(f64::from_bits(self.u64()?))
    }

    fn bool(&mut self) -> Result<bool, DrillError> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            other => Err(DrillError::corrupt(format!("flag byte {other}"))),
        }
    }

    fn text(&mut self) -> Result<String, DrillError> {
        self.dec
            .get_str(MAX_TEXT_BYTES)
            .map(str::to_string)
            .map_err(|e| DrillError::corrupt(e.to_string()))
    }

    fn opt<T>(
        &mut self,
        read: impl FnOnce(&mut Self) -> Result<T, DrillError>,
    ) -> Result<Option<T>, DrillError> {
        if self.bool()? {
            read(self).map(Some)
        } else {
            Ok(None)
        }
    }

    fn ids(&mut self) -> Result<Vec<String>, DrillError> {
        self.dec
            .get_canonical_seq(MAX_LIST_ITEMS, |d| {
                d.get_str(MAX_ID_BYTES).map(str::to_string)
            })
            .map_err(|e| DrillError::corrupt(e.to_string()))
    }

    fn position(&mut self) -> Result<Position, DrillError> {
        Ok(Position {
            crs: self.text()?,
            easting_m: self.f64()?,
            northing_m: self.f64()?,
            elevation_m: self.f64()?,
            method: PositionMethod::from_tag(self.u8()?)?,
            sigma_horizontal_m: self.f64()?,
            sigma_vertical_m: self.f64()?,
        })
    }

    fn record(&mut self) -> Result<DrillRecord, DrillError> {
        let kind = RecordKind::from_tag(self.u8()?)?;
        Ok(match kind {
            RecordKind::Collar => DrillRecord::Collar(Collar {
                hole_id: self.text()?,
                hole_type: HoleType::from_tag(self.u8()?)?,
                program: self.text()?,
                planned: self.opt(Self::position)?,
                actual: self.opt(Self::position)?,
            }),
            RecordKind::Survey => DrillRecord::Survey(Survey {
                survey_id: self.text()?,
                hole_id: self.text()?,
                depth_mm: self.u64()?,
                instrument: SurveyInstrument::from_tag(self.u8()?)?,
                orientation: self.opt(|r| {
                    Ok(Orientation {
                        azimuth_deg: r.f64()?,
                        dip_deg: r.f64()?,
                        sigma_azimuth_deg: r.f64()?,
                        sigma_dip_deg: r.f64()?,
                    })
                })?,
                magnetic_interference: self.bool()?,
                run: RunDirection::from_tag(self.u8()?)?,
            }),
            RecordKind::Interval => DrillRecord::Interval(Interval {
                interval_id: self.text()?,
                hole_id: self.text()?,
                from_mm: self.u64()?,
                to_mm: self.u64()?,
                recovery: self.opt(|r| {
                    Ok(Recovery {
                        percent: r.f64()?,
                        method: RecoveryMethod::from_tag(r.u8()?)?,
                    })
                })?,
                lithology: self.text()?,
                alteration: self.opt(|r| {
                    Ok(Alteration {
                        kind: r.text()?,
                        intensity: r.text()?,
                    })
                })?,
                mineralization: self.text()?,
                weathering: self.text()?,
                logger: match self.u8()? {
                    1 => Logger::Human {
                        logger_id: self.text()?,
                    },
                    2 => Logger::Scanner {
                        instrument_id: self.text()?,
                    },
                    3 => Logger::Model {
                        name: self.text()?,
                        version: self.text()?,
                    },
                    other => return Err(DrillError::corrupt(format!("logger tag {other}"))),
                },
            }),
            RecordKind::Sample => DrillRecord::Sample(Sample {
                sample_id: self.text()?,
                interval_ids: self.ids()?,
                mass_g: self.opt(Self::f64)?,
                sample_type: SampleType::from_tag(self.u8()?)?,
                splitter: self.text()?,
                custody: self
                    .dec
                    .get_canonical_seq(MAX_LIST_ITEMS, |d| {
                        Ok(CustodyEvent {
                            at_ns: d.get_i64()?,
                            holder: d.get_str(MAX_TEXT_BYTES)?.to_string(),
                        })
                    })
                    .map_err(|e| DrillError::corrupt(e.to_string()))?,
            }),
            RecordKind::Dispatch => DrillRecord::Dispatch(Dispatch {
                dispatch_id: self.text()?,
                lab: self.text()?,
                sample_ids: self.ids()?,
                dispatched_at_ns: self.i64()?,
            }),
            RecordKind::AssayResult => DrillRecord::AssayResult(AssayResult {
                result_id: self.text()?,
                sample_id: self.text()?,
                dispatch_id: self.text()?,
                analyte: self.text()?,
                method: AssayMethod::from_tag(self.u8()?)?,
                value: {
                    let tag = self.u8()?;
                    let v = self.f64()?;
                    match tag {
                        1 => AssayValue::Measured { mass_fraction: v },
                        2 => AssayValue::BelowDetection { limit: v },
                        3 => AssayValue::AboveUpper { limit: v },
                        other => {
                            return Err(DrillError::corrupt(format!("assay value tag {other}")));
                        }
                    }
                },
                lab: self.text()?,
                lab_batch: self.text()?,
                certificate: match self.u8()? {
                    1 => CertificateRef::Certificate {
                        certificate_id: self.text()?,
                    },
                    2 => CertificateRef::LimsDirectNoCertificate,
                    other => return Err(DrillError::corrupt(format!("certificate tag {other}"))),
                },
                qaqc_regime: QaqcRegime::from_tag(self.u8()?)?,
                received_at_ns: self.i64()?,
            }),
            RecordKind::QcInsertion => DrillRecord::QcInsertion(QcInsertion {
                insertion_id: self.text()?,
                sample_id: self.text()?,
                kind: match self.u8()? {
                    1 => QcKind::Crm {
                        crm_id: self.text()?,
                        certified_mass_fraction: self.f64()?,
                        sigma_c: self.f64()?,
                        sigma_l: self.f64()?,
                        certificate_source: self.text()?,
                    },
                    2 => QcKind::Blank {
                        stage: BlankStage::from_tag(self.u8()?)?,
                    },
                    3 => QcKind::Duplicate {
                        stage: DuplicateStage::from_tag(self.u8()?)?,
                        original_sample_id: self.text()?,
                    },
                    4 => QcKind::Umpire {
                        original_sample_id: self.text()?,
                        umpire_lab: self.text()?,
                    },
                    other => return Err(DrillError::corrupt(format!("qc kind tag {other}"))),
                },
            }),
            RecordKind::LabCertificate => DrillRecord::LabCertificate(LabCertificate {
                certificate_id: self.text()?,
                lab: self.text()?,
                original_sha256: Digest32(
                    self.dec
                        .get_fixed::<32>()
                        .map_err(|e| DrillError::corrupt(e.to_string()))?,
                ),
                cas_root: Digest32(
                    self.dec
                        .get_fixed::<32>()
                        .map_err(|e| DrillError::corrupt(e.to_string()))?,
                ),
                media_type: self.text()?,
                issued_at_ns: self.i64()?,
            }),
            RecordKind::SampleLoss => DrillRecord::SampleLoss(SampleLoss {
                loss_id: self.text()?,
                dispatch_id: self.text()?,
                sample_id: self.text()?,
                declared_at_ns: self.i64()?,
                finding: self.text()?,
            }),
        })
    }
}

/// Read one ledger version written by [`encode_version`].
///
/// # Errors
/// [`DrillError::Corrupt`] for bytes that are not a whole `DRLR` frame, or whose fields
/// [`encode_version`] would refuse or write differently (for example `-0.0`, or trailing bytes).
pub fn decode_version(bytes: &[u8]) -> Result<RecordVersion, DrillError> {
    let mut dec = Decoder::open_frame(bytes, RECORD_TAG, RECORD_VERSION, RECORD_VERSION)
        .map_err(|e| DrillError::corrupt(e.to_string()))?;
    let mut reader = Reader { dec: &mut dec };
    let record = reader.record()?;
    let correction = reader.opt(Reader::text)?;
    dec.finish()
        .map_err(|e| DrillError::corrupt(e.to_string()))?;
    let again = encode_version(&record, correction.as_deref())
        .map_err(|e| DrillError::corrupt(format!("decoded record is refused: {e}")))?;
    if again != bytes {
        return Err(DrillError::corrupt(
            "the bytes are not the canonical encoding of the record they hold",
        ));
    }
    Ok(RecordVersion { record, correction })
}

#[cfg(test)]
pub(crate) mod tests;
