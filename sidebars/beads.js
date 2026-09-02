// Beads — official GUI inside cmux (`beads`).
// Restricted JS scene (cmux docs/custom-sidebars.md). Native right-sidebar
// panel, not an iframe and not a PTY stuffed in a pane.
// Chrome matches built-in right-sidebar examples: glass surface, 14pt title,
// 10/13 type, 8–10pt continuous corners, host hover wash, Reorderable.
// Bind only live cmux context. Taps run cmux() only. No bd, no filesystem.

const MAX_WORKSPACES = 40;
const MAX_BEADS = 24;
const MAX_TABS = 12;
const WASH = "#7f7f7f24";
const WASH_SOFT = "#7f7f7f1c";
const WASH_STRONG = "#7f7f7f3d";
const CARD = "#7f7f7f14";
const CARD_HOVER = "#7f7f7f28";
const UNREAD = "#E4573D";

const BEAD_COLUMNS = [
  "open",
  "in_progress",
  "blocked",
  "deferred",
  "pinned",
  "hooked",
  "closed",
];

let selectOverride = null;
const [selectTick, setSelectTick] = signal(0);
let orderOverride = null;
const [orderTick, setOrderTick] = signal(0);

function hasText(value) {
  return value != null && value !== "";
}

function liveWorkspaces() {
  selectTick();
  orderTick();
  let ws = data.workspaces() ?? [];
  if (orderOverride) {
    const actual = ws.map((w) => w.id).join(",");
    const wanted = orderOverride.filter((id) => ws.some((w) => w.id === id)).join(",");
    if (actual === wanted) {
      orderOverride = null;
    } else {
      const rank = new Map(orderOverride.map((id, i) => [id, i]));
      ws = [...ws].sort((a, b) => (rank.get(a.id) ?? 1e9) - (rank.get(b.id) ?? 1e9));
    }
  }
  return ws.slice(0, MAX_WORKSPACES);
}

function isSelected(w) {
  selectTick();
  if (!w) return false;
  if (selectOverride) {
    if (data.selectedId() === selectOverride) selectOverride = null;
    else return w.id === selectOverride;
  }
  return !!w.selected;
}

function selectWorkspace(id) {
  if (!id) return;
  selectOverride = id;
  setSelectTick(selectTick() + 1);
  cmux("workspace.select", { workspace_id: id });
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

function tabFocusId(t) {
  if (hasText(t.surfaceId)) return t.surfaceId;
  return t.id;
}

function selectedWorkspaces() {
  return liveWorkspaces().filter((w) => isSelected(w)).slice(0, 1);
}

function selectedWorkspace() {
  return selectedWorkspaces()[0] ?? null;
}

function beadCount() {
  const selected = selectedWorkspace();
  if (!selected) return 0;
  return beadStatuses(selected).length;
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
  if (col === "in_progress") return "IN PROGRESS";
  return String(col).toUpperCase();
}

function handleMove(id, index) {
  const ws = liveWorkspaces();
  const rest = ws.map((w) => w.id).filter((wid) => wid !== id);
  let insertAt = index;
  if (insertAt < 0) insertAt = 0;
  if (insertAt > rest.length) insertAt = rest.length;
  orderOverride = [...rest.slice(0, insertAt), id, ...rest.slice(insertAt)];
  setOrderTick(orderTick() + 1);
  cmux("workspace.reorder", { workspace_id: id, index: index });
}

function workspaceMenu(w) {
  const act = (action) => () =>
    cmux("workspace.action", { action: action, workspace_id: w().id });
  return [
    Button("Select", () => selectWorkspace(w().id)),
    Button(() => (w().pinned ? "Unpin" : "Pin"), () =>
      cmux("workspace.action", {
        action: w().pinned ? "unpin" : "pin",
        workspace_id: w().id,
      }),
    ),
    Button(() => (w().unread > 0 ? "Mark as Read" : "Mark as Unread"), () =>
      cmux("workspace.action", {
        action: w().unread > 0 ? "mark_read" : "mark_unread",
        workspace_id: w().id,
      }),
    ),
    Divider(),
    Menu("Move", [
      Button("Move Up", act("move_up")),
      Button("Move Down", act("move_down")),
      Button("Move to Top", act("move_top")),
    ]),
  ];
}

function unreadBadge(countFn) {
  return Text(() => (countFn() > 0 ? String(countFn()) : ""))
    .font(10)
    .bold()
    .color("white")
    .paddingHorizontal(() => (countFn() > 0 ? 5 : 0))
    .paddingVertical(() => (countFn() > 0 ? 1 : 0))
    .background(() => (countFn() > 0 ? UNREAD : null))
    .cornerRadius(7);
}

function beadCard(s, w) {
  return HStack({ spacing: 0 }, [
    RoundedRectangle({ width: 3, cornerRadius: 2 })
      .fill(() => chipTint(s()))
      .frame({ height: 36 }),
    VStack({ spacing: 2 }, [
      Text(() => beadTitle(s()))
        .font(13)
        .weight("semibold")
        .lineLimit(1)
        .truncation("tail")
        .marquee()
        .color("primary"),
      Text(() => beadStatusName(s()))
        .font(10)
        .monospaced()
        .color(() => chipTint(s()))
        .lineLimit(1),
    ]).paddingLeading(9),
    Spacer({ minLength: 0 }),
  ])
    .paddingHorizontal(10)
    .paddingVertical(7)
    .cornerRadius(10)
    .background(CARD)
    .hoverBackground(CARD_HOVER)
    .frame({ maxWidth: "infinity" })
    .onTap(() => selectWorkspace(w().id))
    .contextMenu([
      Button("Select host", () => selectWorkspace(w().id)),
    ]);
}

function workspaceRow(w) {
  return HStack({ spacing: 8 }, [
    Image("line.3.horizontal").font(9).color("tertiary"),
    Circle({ size: 7 }).fill(() => (isSelected(w()) ? "accent" : "tertiary")),
    Text(() => w().title)
      .font(13)
      .lineLimit(1)
      .truncation("tail")
      .marquee()
      .color(() => (isSelected(w()) ? "primary" : "secondary")),
    Spacer({ minLength: 0 }),
    Image("pin.fill")
      .font(8)
      .color("tertiary")
      .opacity(() => (w().pinned && !(w().unread > 0) ? 1 : 0)),
    unreadBadge(() => w().unread ?? 0),
  ])
    .paddingHorizontal(10)
    .paddingVertical(6)
    .cornerRadius(8)
    .background(() => (isSelected(w()) ? WASH_STRONG : null))
    .hoverBackground(() => (isSelected(w()) ? WASH_STRONG : WASH))
    .frame({ maxWidth: "infinity" })
    .onTap(() => selectWorkspace(w().id))
    .contextMenu(workspaceMenu(w));
}

function tabRow(t) {
  return HStack({ spacing: 6 }, [
    Circle({ size: 6 }).fill(() => (t().focused ? "accent" : "tertiary")),
    Text(() => t().title)
      .font(12)
      .lineLimit(1)
      .truncation("tail")
      .color(() => (t().focused ? "primary" : "secondary")),
    Spacer({ minLength: 0 }),
  ])
    .paddingHorizontal(10)
    .paddingVertical(5)
    .cornerRadius(7)
    .hoverBackground(WASH)
    .frame({ maxWidth: "infinity" })
    .onTap(() => cmux("surface.focus", { surface_id: tabFocusId(t()) }));
}

function kanbanColumn(section, w) {
  return VStack({ spacing: 4 }, [
    HStack({ spacing: 6 }, [
      Text(() => columnTitle(section().col))
        .font(10)
        .weight("semibold")
        .color("tertiary"),
      Spacer(),
      Text(() => String((section().items ?? []).length))
        .font(10)
        .monospaced()
        .color("tertiary"),
    ]).paddingHorizontal(10),
    ForEach(
      {
        items: () => (section().items ?? []).slice(0, MAX_BEADS),
        key: (s) => s.key ?? s.value,
      },
      (s) => beadCard(s, w),
    ),
  ]);
}

function beadsBoard(w) {
  return VStack({ spacing: 8 }, [
    Text(() =>
      beadStatuses(w()).length === 0
        ? "Run cmux-beads watch to load the Beads board."
        : "",
    )
      .font(11)
      .color("tertiary")
      .paddingHorizontal(10)
      .lineLimit(3),
    ForEach(
      {
        items: () => kanbanSections(w()),
        key: (s) => s.id,
      },
      (section) => kanbanColumn(section, w),
    ),
  ]);
}

function surfaces(w) {
  return VStack({ spacing: 4 }, [
    HStack({ spacing: 6 }, [
      Text("SURFACES").font(10).weight("semibold").color("tertiary"),
      Spacer(),
      Text(() => String((w().tabs ?? []).length))
        .font(10)
        .monospaced()
        .color("tertiary"),
    ]).paddingHorizontal(10),
    ForEach(
      {
        items: () => (w().tabs ?? []).slice(0, MAX_TABS),
        key: (t) => t.id,
      },
      (t) => tabRow(t),
    ),
  ]);
}

function hostHeader() {
  return HStack({ spacing: 6 }, [
    Text("HOST").font(10).weight("semibold").color("tertiary"),
    Spacer(),
    Text(() => String(liveWorkspaces().length))
      .font(10)
      .monospaced()
      .color("tertiary"),
  ]).paddingHorizontal(10);
}

function selectedHeader() {
  return HStack({ spacing: 8 }, [
    Text(() => data.selectedTitle() ?? "")
      .font(13)
      .weight("semibold")
      .lineLimit(1)
      .truncation("tail")
      .marquee()
      .color("primary"),
    Spacer(),
    unreadBadge(() => selectedWorkspace()?.unread ?? 0),
  ])
    .paddingHorizontal(10)
    .paddingVertical(() => (data.selectedTitle() ? 8 : 0))
    .cornerRadius(10)
    .background(() => (data.selectedTitle() ? WASH : null))
    .hoverBackground(() => (data.selectedTitle() ? WASH_SOFT : null))
    .frame({ maxWidth: "infinity" });
}

sidebar(
  () =>
    VStack({ spacing: 8 }, [
      HStack({ spacing: 6 }, [
        Text("Beads").font(14).weight("semibold").color("primary"),
        Spacer(),
        Text(() => (beadCount() ? String(beadCount()) : ""))
          .font(11)
          .monospaced()
          .color("tertiary"),
      ]).paddingHorizontal(10),
      selectedHeader(),
      ForEach(
        {
          items: selectedWorkspaces,
          key: (w) => "board:" + w.id,
        },
        (w) => beadsBoard(w),
      ),
      hostHeader(),
      Text(() => (liveWorkspaces().length === 0 ? "No live host workspace" : ""))
        .font(11)
        .color("tertiary")
        .paddingHorizontal(10),
      Reorderable(
        {
          items: liveWorkspaces,
          key: (w) => w.id,
          spacing: 2,
          onMove: handleMove,
        },
        (w) => workspaceRow(w),
      ),
      ForEach(
        {
          items: selectedWorkspaces,
          key: (w) => "surf:" + w.id,
        },
        (w) => surfaces(w),
      ),
      Text("Beads board updates after cmux-beads sync or watch.")
        .font(11)
        .color("tertiary")
        .paddingHorizontal(10)
        .lineLimit(3),
      Spacer(),
    ]).paddingHorizontal(6),
  { surface: "glass" },
);
