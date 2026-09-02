// Beads — official GUI inside cmux (`beads`).
// Restricted JS scene (cmux docs/custom-sidebars.md). Native right-sidebar
// panel, not an iframe and not a PTY stuffed in a pane. Root is sidebar(() => view).
// Bind only live cmux context: workspaces, tabs, statuses, agents, progress, git, color.
// The Beads board is the product. Host workspaces are the switcher.
// Bead rows appear after `cmux-beads sync` / `watch` writes bead:<id> keys.
// Buttons / onTap run cmux() only. No bd, no filesystem, no invented team.

const MAX_WORKSPACES = 40;
const MAX_BEADS = 24;
const MAX_TABS = 12;

const BEAD_COLUMNS = [
  "open",
  "in_progress",
  "blocked",
  "deferred",
  "pinned",
  "hooked",
  "closed",
];

function hasText(value) {
  return value != null && value !== "";
}

function liveWorkspaces() {
  return (data.workspaces() ?? []).slice(0, MAX_WORKSPACES);
}

function isBeadStatus(s) {
  return hasText(s.key) && String(s.key).indexOf("bead:") === 0;
}

function beadStatuses(w) {
  return (w.statuses ?? []).filter(isBeadStatus).slice(0, MAX_BEADS);
}

function columnOf(s) {
  const raw = hasText(s.value) ? String(s.value) : "";
  const head = raw.split(" · ")[0];
  for (let i = 0; i < BEAD_COLUMNS.length; i += 1) {
    if (head === BEAD_COLUMNS[i]) return BEAD_COLUMNS[i];
  }
  return "open";
}

function chipLabel(s) {
  if (hasText(s.value)) return s.value;
  if (hasText(s.key)) return s.key;
  return "";
}

function beadTitle(s) {
  const raw = chipLabel(s);
  const sep = " · ";
  const at = raw.indexOf(sep);
  if (at >= 0) return raw.slice(at + sep.length);
  return raw;
}

function beadStatusName(s) {
  const raw = chipLabel(s);
  const sep = " · ";
  const at = raw.indexOf(sep);
  if (at >= 0) return raw.slice(0, at);
  return columnOf(s);
}

function chipTint(s) {
  if (hasText(s.color)) return s.color;
  return "accent";
}

function rowTint(w) {
  if (hasText(w.color)) return w.color;
  if (w.selected) return "accent";
  if (w.unread > 0) return "accent";
  return "secondary";
}

function tabFocusId(t) {
  if (hasText(t.surfaceId)) return t.surfaceId;
  return t.id;
}

function selectedWorkspaces() {
  return liveWorkspaces().filter((w) => w.selected).slice(0, 1);
}

function beadsInColumn(w, col) {
  return beadStatuses(w).filter((s) => columnOf(s) === col);
}

function kanbanSections(w) {
  const out = [];
  for (let i = 0; i < BEAD_COLUMNS.length; i += 1) {
    const col = BEAD_COLUMNS[i];
    const items = beadsInColumn(w, col);
    if (items.length > 0) out.push({ id: col, col: col, items: items });
  }
  return out.slice(0, 7);
}

function columnTitle(col) {
  if (col === "in_progress") return "in progress";
  return col;
}

function beadCard(s) {
  return HStack({ spacing: 8 }, [
    Circle({ size: 7 }).fill(() => chipTint(s())),
    VStack({ spacing: 1 }, [
      Text(() => beadTitle(s()))
        .font(12)
        .weight("semibold")
        .color("primary")
        .lineLimit(2),
      Text(() => beadStatusName(s()))
        .font(10)
        .monospaced()
        .color("tertiary")
        .lineLimit(1),
    ]),
    Spacer(),
  ])
    .paddingVertical(6)
    .paddingHorizontal(8)
    .cornerRadius(8)
    .hoverBackground("accent")
    .frame({ maxWidth: "infinity" });
}

function workspaceRow(w) {
  return HStack({ spacing: 8 }, [
    Circle({ size: 7 }).fill(() => (w().selected ? rowTint(w()) : "tertiary")),
    Text(() => w().title)
      .font(12)
      .weight(() => (w().selected ? "semibold" : "regular"))
      .color("primary")
      .lineLimit(1),
    Spacer(),
    Text(() => String(w().tabCount ?? 0))
      .font(10)
      .monospaced()
      .color("secondary"),
  ])
    .paddingVertical(5)
    .paddingHorizontal(6)
    .cornerRadius(6)
    .hoverBackground("accent")
    .onTap(() => cmux("workspace.select", { workspace_id: w().id }))
    .contextMenu([
      Button("Select", () => cmux("workspace.select", { workspace_id: w().id })),
      Button(() => (w().pinned ? "Unpin" : "Pin"), () =>
        cmux("workspace.action", {
          action: w().pinned ? "unpin" : "pin",
          workspace_id: w().id,
        }),
      ),
    ]);
}

function tabRow(t) {
  return HStack({ spacing: 6 }, [
    Circle({ size: 6 }).fill(() => (t().focused ? "accent" : "tertiary")),
    Text(() => t().title)
      .font(11)
      .color(() => (t().focused ? "primary" : "secondary"))
      .lineLimit(1),
    Spacer(),
  ])
    .paddingVertical(3)
    .paddingHorizontal(6)
    .cornerRadius(4)
    .hoverBackground("accent")
    .onTap(() => cmux("surface.focus", { surface_id: tabFocusId(t()) }));
}

function kanbanColumn(section) {
  return VStack({ spacing: 4 }, [
    HStack({ spacing: 6 }, [
      Text(() => columnTitle(section().col))
        .font(10)
        .weight("semibold")
        .color("secondary"),
      Spacer(),
      Text(() => String((section().items ?? []).length))
        .font(10)
        .monospaced()
        .color("tertiary"),
    ]),
    ForEach(
      {
        items: () => (section().items ?? []).slice(0, MAX_BEADS),
        key: (s) => s.key ?? s.value,
      },
      (s) => beadCard(s),
    ),
  ]);
}

function beadsBoard(w) {
  return VStack({ spacing: 8 }, [
    Text(() => (beadStatuses(w()).length === 0 ? "Run cmux-beads watch to load the Beads board." : ""))
      .font(11)
      .color("tertiary")
      .lineLimit(3),
    ForEach(
      {
        items: () => kanbanSections(w()),
        key: (s) => s.id,
      },
      (section) => kanbanColumn(section),
    ),
  ]);
}

function hostAndSurfaces(w) {
  return VStack({ spacing: 6 }, [
    Text("Surfaces").font(10).weight("semibold").color("tertiary"),
    ForEach(
      {
        items: () => (w().tabs ?? []).slice(0, MAX_TABS),
        key: (t) => t.id,
      },
      (t) => tabRow(t),
    ),
  ]);
}

function reorderWorkspaces(id, index) {
  cmux("workspace.reorder", { workspace_id: id, index: index });
}

sidebar(
  () =>
    VStack({ spacing: 10 }, [
      HStack({ spacing: 8 }, [
        Text("Beads").font("headline").weight("semibold").color("accent"),
        Spacer(),
        Text(() => (data.clock() ? data.clock().time : ""))
          .font(10)
          .monospaced()
          .color("tertiary"),
      ]),
      Text(() => data.selectedTitle() ?? "")
        .font(11)
        .color("secondary")
        .lineLimit(1),
      Divider(),
      ForEach(
        {
          items: selectedWorkspaces,
          key: (w) => "board:" + w.id,
        },
        (w) => beadsBoard(w),
      ),
      Divider(),
      HStack({}, [
        Text("Host").font("caption").weight("semibold").color("secondary"),
        Spacer(),
        Text(() => String(data.workspaceCount() ?? 0))
          .font(10)
          .monospaced()
          .color("tertiary"),
      ]),
      Text(() => (liveWorkspaces().length === 0 ? "No live host workspace" : ""))
        .font("caption")
        .color("tertiary"),
      Reorderable(
        {
          items: liveWorkspaces,
          key: (w) => w.id,
          onMove: reorderWorkspaces,
        },
        (w) => workspaceRow(w),
      ),
      ForEach(
        {
          items: selectedWorkspaces,
          key: (w) => "surf:" + w.id,
        },
        (w) => hostAndSurfaces(w),
      ),
      Divider(),
      Text("Beads board updates after cmux-beads sync or watch.")
        .font("caption")
        .color("tertiary")
        .lineLimit(3),
    ]).padding(12),
  { surface: "glass" },
);
