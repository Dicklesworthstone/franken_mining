//! The region table read against the plan's §26.6 tree, its derived rules, the role profiles and the contexts.

use std::collections::BTreeMap;
use std::process::Command;

use super::*;
use crate::WallClock;

/// One region as the plan's §26.6 tree draws it.
#[derive(Debug)]
struct Drawn {
    parent: Option<String>,
    caps: Option<Mask>,
    supervisor: Option<Supervisor>,
    instances: Instances,
    only: Option<Role>,
}

/// The tree in the plan's §26.6 code block, by region name without its `Region` suffix, in drawing order. A
/// region's parent is the nearest line above it whose name starts further left.
fn drawn_tree() -> Vec<(String, Drawn)> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md"
    );
    let plan = std::fs::read_to_string(path).unwrap();
    let section = &plan[plan.find("### 26.6").unwrap()..];
    let block = &section[section.find("```text\n").unwrap() + 8..];
    let block = &block[..block.find("```").unwrap()];
    let mut out: Vec<(String, Drawn)> = Vec::new();
    let mut stack: Vec<(usize, String)> = Vec::new();
    for line in block.lines() {
        let chars: Vec<char> = line.chars().collect();
        let Some(col) = chars.iter().position(char::is_ascii_alphabetic) else {
            continue;
        };
        let word: String = chars[col..]
            .iter()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect();
        let name = word.strip_suffix("Region").unwrap().to_string();
        let after: String = chars[col + word.chars().count()..].iter().collect();
        let instances = if after.starts_with('*') {
            Instances::Many
        } else if let Some(rest) = after.strip_prefix('{')
            && let Some(end) = rest.find('}')
            && rest[end + 1..].starts_with('*')
        {
            Instances::PerKey(Box::leak(rest[..end].to_string().into_boxed_str()))
        } else {
            Instances::One
        };
        let caps = line.find("CapSet<").map(|at| {
            let digits: Vec<bool> = line[at + 7..line[at..].find('>').unwrap() + at]
                .split(',')
                .map(|d| d.trim() == "1")
                .collect();
            Mask::of([digits[0], digits[1], digits[2], digits[3], digits[4]])
        });
        let supervisor = [
            ("rest_for_one", Supervisor::RestForOne),
            ("one_for_one", Supervisor::OneForOne),
            ("one_for_all", Supervisor::OneForAll),
        ]
        .into_iter()
        .find(|(w, _)| line.contains(w))
        .map(|(_, s)| s);
        let only = if line.contains("(site)") {
            Some(Role::Site)
        } else if line.contains("(enterprise)") {
            Some(Role::Enterprise)
        } else {
            None
        };
        while stack.last().is_some_and(|(c, _)| *c >= col) {
            stack.pop();
        }
        let parent = stack.last().map(|(_, n)| n.clone());
        stack.push((col, name.clone()));
        out.push((
            name,
            Drawn {
                parent,
                caps,
                supervisor,
                instances,
                only,
            },
        ));
    }
    out
}

/// A region's role is inherited: Telemetry runs only where Ingest runs.
fn inherited_role(tree: &[(String, Drawn)], name: &str) -> Option<Role> {
    let by_name: BTreeMap<&str, &Drawn> = tree.iter().map(|(n, d)| (n.as_str(), d)).collect();
    let mut at = by_name[name];
    loop {
        if at.only.is_some() {
            return at.only;
        }
        at = by_name[at.parent.as_deref()?];
    }
}

#[test]
fn the_table_is_the_plans_tree() {
    let tree = drawn_tree();
    let names: Vec<String> = REGIONS.iter().map(|r| format!("{:?}", r.kind)).collect();
    let drawn: Vec<&String> = tree.iter().map(|(n, _)| n).collect();
    assert_eq!(drawn, names.iter().collect::<Vec<_>>());
    for ((name, d), spec) in tree.iter().zip(REGIONS) {
        let parent = spec.parent.map(|p| format!("{p:?}"));
        assert_eq!(d.parent, parent, "{name}'s parent");
        if let Some(caps) = d.caps {
            assert_eq!(caps, spec.caps, "{name}'s CapSet");
        }
        assert_eq!(d.supervisor, spec.supervisor, "{name}'s supervisor");
        assert_eq!(d.instances, spec.instances, "{name}'s instances");
        assert_eq!(inherited_role(&tree, name), spec.only, "{name}'s role");
    }
    // The plan draws a set on every leaf and on no interior region.
    for (name, d) in &tree {
        let leaf = !tree.iter().any(|(_, c)| c.parent.as_deref() == Some(name));
        assert_eq!(d.caps.is_some(), leaf, "{name}");
    }
}

#[test]
fn every_row_is_its_variant_and_its_type() {
    assert_eq!(REGIONS.len(), 18);
    for (i, r) in REGIONS.iter().enumerate() {
        assert_eq!(r.kind as usize, i);
        assert_eq!(r.kind.spec(), r);
    }
    fn row<R: Region>() {
        assert_eq!(
            <R::Caps as Caps>::MASK,
            R::KIND.spec().caps,
            "{:?}",
            R::KIND
        );
    }
    row::<ProcessRegion>();
    row::<NodeRegion>();
    row::<AuthorityRegion>();
    row::<LedgerWriterRegion>();
    row::<ObjectStoreRegion>();
    row::<IngestRegion>();
    row::<TelemetryRegion>();
    row::<ImportQuarantineRegion>();
    row::<ComputeRegion>();
    row::<LawRegion>();
    row::<EstimationRunRegion>();
    row::<ProjectionRegion>();
    row::<SyncRegion>();
    row::<PeerSessionRegion>();
    row::<TransferRegion>();
    row::<FinanceBridgeRegion>();
    row::<EffectRegion>();
    row::<OperationsRegion>();
}

#[test]
fn an_interior_set_is_the_union_of_its_childrens_and_the_root_holds_all() {
    assert_eq!(RegionKind::Process.spec().caps, Mask::ALL);
    for r in REGIONS.iter().filter(|r| r.kind != RegionKind::Process) {
        let children: Vec<Mask> = REGIONS
            .iter()
            .filter(|c| c.parent == Some(r.kind))
            .map(|c| c.caps)
            .collect();
        if !children.is_empty() {
            let union = children.into_iter().fold(Mask::NONE, Mask::union);
            assert_eq!(r.caps, union, "{:?}", r.kind);
        }
        let parent = r.parent.unwrap().spec().caps;
        assert!(r.caps.within(parent), "{:?}", r.kind);
    }
}

#[test]
fn random_stays_at_the_root_and_remote_on_the_sync_path() {
    let holding = |f: fn(&Mask) -> bool| -> Vec<RegionKind> {
        REGIONS
            .iter()
            .filter(|r| f(&r.caps))
            .map(|r| r.kind)
            .collect()
    };
    assert_eq!(holding(|m| m.random), [RegionKind::Process]);
    use RegionKind::{Node, PeerSession, Process, Sync, Transfer};
    assert_eq!(
        holding(|m| m.remote),
        [Process, Node, Sync, PeerSession, Transfer]
    );
}

#[test]
fn engines_get_no_time_io_or_remote() {
    // PO-BED-8, its compile-time half: the law region holds nothing and an estimation run SPAWN alone.
    assert_eq!(RegionKind::Law.spec().caps, Mask::NONE);
    assert_eq!(
        RegionKind::EstimationRun.spec().caps,
        Mask::of([true, false, false, false, false])
    );
    let compute = Cx::ambient_root()
        .open::<NodeRegion>()
        .open::<ComputeRegion>();
    let law = compute.open::<LawRegion>();
    assert_eq!(law.caps(), Mask::NONE);
    assert_eq!(
        compute.open::<EstimationRunRegion>().caps(),
        RegionKind::EstimationRun.spec().caps
    );
    assert_eq!(law.region(), RegionKind::Law);
}

#[test]
fn each_role_runs_a_whole_tree() {
    for role in Role::ALL {
        let p = profile(role);
        assert_eq!(p[0].kind, RegionKind::Process);
        for r in &p[1..] {
            let parent = r.parent.unwrap();
            assert!(p.iter().any(|q| q.kind == parent), "{role:?}: {:?}", r.kind);
        }
        let has = |k: RegionKind| p.iter().any(|r| r.kind == k);
        assert_eq!(has(RegionKind::Ingest), role == Role::Site, "{role:?}");
        assert_eq!(has(RegionKind::Telemetry), role == Role::Site, "{role:?}");
        assert_eq!(
            has(RegionKind::ImportQuarantine),
            role == Role::Site,
            "{role:?}"
        );
        assert_eq!(
            has(RegionKind::FinanceBridge),
            role == Role::Enterprise,
            "{role:?}"
        );
        let n = 18 - 3 * usize::from(role != Role::Site) - usize::from(role != Role::Enterprise);
        assert_eq!(p.len(), n, "{role:?}");
    }
}

#[test]
fn contexts_narrow_down_the_tree_and_use_what_they_hold() {
    let node = Cx::ambient_root().open::<NodeRegion>();
    assert_eq!(node.caps(), Mask::of([true, true, false, true, true]));
    let writer = node.open::<AuthorityRegion>().open::<LedgerWriterRegion>();
    assert_eq!(writer.region(), RegionKind::LedgerWriter);
    assert!(writer.clock().now_ns().unwrap() > 0);
    // Narrowing keeps the region and drops capabilities; the store's set has IO but not TIME.
    let store = writer.narrow::<<ObjectStoreRegion as Region>::Caps>();
    assert_eq!(store.region(), RegionKind::LedgerWriter);
    assert_eq!(store.caps(), RegionKind::ObjectStore.spec().caps);
    let mut cat = Command::new("sh");
    cat.args(["-c", "cat"]);
    let out = store.run(&mut cat, b"held", 16).unwrap();
    assert_eq!(out.output.stdout, b"held");
    // Any context may give everything up, as for a call into an engine.
    let none = Cx::ambient_root().narrow::<CapSet<false, false, false, false, false>>();
    assert_eq!(none.caps(), Mask::NONE);
}

#[test]
fn mask_within_and_union_follow_the_bits() {
    let all: Vec<Mask> = (0..32_u8)
        .map(|b| Mask::of([0, 1, 2, 3, 4].map(|i| (b >> i) & 1 == 1)))
        .collect();
    for &a in &all {
        for &b in &all {
            let bitwise = (0..5).all(|i| !a.bits()[i] || b.bits()[i]);
            assert_eq!(a.within(b), bitwise, "{a:?} within {b:?}");
            let u = a.union(b);
            assert!(a.within(u) && b.within(u));
            assert!(
                all.iter()
                    .filter(|m| a.within(**m) && b.within(**m))
                    .all(|m| u.within(*m))
            );
        }
    }
}
