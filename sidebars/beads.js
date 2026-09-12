// CONTRIB / LEGACY — NOT THE PRODUCT.
// Interpreted custom-sidebar scene for the generic Custom slot
// (`cmux right-sidebar set custom beads`). Kept in-tree for reference only.
//
// The product is Beads as a tab on the existing right sidebar, a sibling of
// Files / Find / Dock:
//   cmux right-sidebar set beads                       host tab (built-in)
//   cmux sidebar plugin install <cmux-beads repo>.git  plugin package
//   cmux sidebar plugin use cmux-beads
//
// Restricted JS scene (cmux docs/custom-sidebars.md). Native right-sidebar
// rendering, not an iframe and not a PTY stuffed in a pane.
// Chrome matches built-in right-sidebar siblings (Files / Find / Dock) and
// host examples (panel-todo, panel-sessions): glass surface, 14pt title,
// 10/11/13 type, 6–8pt continuous corners, host hover wash, Reorderable.
// Flat rows — no card fills, shadows, or brand palette. Scope chips match
// panel-sessions. Bind only live cmux context. Taps run cmux() only.
// Board / List + Host / Focus / Assigned deepen Beads for the focused
// host / pane / chat.

const MAX_WORKSPACES = 40;
const MAX_BEADS = 24;
const MAX_TABS = 12;
const WASH = "#7f7f7f24";
const WASH_SOFT = "#7f7f7f1c";
const WASH_STRONG = "#7f7f7f3d";
const WASH_FAINT = "#7f7f7f14";
const UNREAD = "#E4573D";
const FOCUS_MARK = "\u25C8";

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
const [viewMode, setViewMode] = signal("board");
const [scopeMode, setScopeMode] = signal("host");

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

function chipLabel(s) {
  if (hasText(s.value)) return s.value;
  if (hasText(s.key)) return s.key;
  return "";
}

function valueParts(s) {
  return String(chipLabel(s)).split(" · ");
}

function focusTagOf(s) {
  const parts = valueParts(s);
  if (parts.length < 2) return null;
  const last = parts[parts.length - 1];
  if (last.indexOf(FOCUS_MARK) === 0) return last.slice(FOCUS_MARK.length);
  return null;
}

function columnOf(s) {
  const parts = valueParts(s);
  const head = parts[0] ?? "";
  for (let i = 0; i < BEAD_COLUMNS.length; i += 1) {
    if (head === BEAD_COLUMNS[i]) return BEAD_COLUMNS[i];
  }
  return "open";
}

function beadTitle(s) {
  const parts = valueParts(s);
  if (parts.length === 0) return "";
  if (parts.length === 1) return parts[0];
  const end = focusTagOf(s) ? parts.length - 1 : parts.length;
  const title = parts.slice(1, end).join(" · ");
  return title || parts[0];
}

function beadStatusName(s) {
  const parts = valueParts(s);
  if (parts.length >= 1 && BEAD_COLUMNS.indexOf(parts[0]) >= 0) return parts[0];
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

function focusedTab(w) {
  return (w.tabs ?? []).find((t) => t.focused) ?? null;
}

function focusedAgent(w) {
  const agents = w.agents ?? [];
  return (
    agents.find((a) => a.status === "working" || a.status === "needs_input") ??
    agents[0] ??
    null
  );
}

function focusLabel(w) {
  const tab = focusedTab(w);
  const agent = focusedAgent(w);
  if (tab && hasText(tab.title) && agent && hasText(agent.name)) {
    return String(tab.title) + " / " + String(agent.name);
  }
  if (tab && hasText(tab.title)) return String(tab.title);
  if (agent && hasText(agent.name)) return String(agent.name);
  return "";
}

function tagMatchesWorkspace(tag, w) {
  if (!tag || !w) return false;
  const ws = String(w.id);
  if (tag === ws) return true;
  return tag.indexOf(ws + "/") === 0;
}

function scopedBeads(w) {
  scopeMode();
  const all = beadStatuses(w);
  const mode = scopeMode();
  if (mode === "host") return all;
  if (mode === "assigned") return all.filter((s) => focusTagOf(s));
  const focused = all.filter((s) => tagMatchesWorkspace(focusTagOf(s), w));
  if (focused.length > 0) return focused;
  return all.filter((s) => focusTagOf(s));
}

function beadCount() {
  const selected = selectedWorkspace();
  if (!selected) return 0;
  return scopedBeads(selected).length;
}

function beadsInColumn(w, col) {
  return scopedBeads(w).filter((s) => columnOf(s) === col);
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

function listSections(w) {
  return kanbanSections(w);
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

// Same control language as panel-sessions scope chips.
function modeChip(label, activeFn, onTap) {
  return Text(label)
    .font(11)
    .weight("semibold")
    .color(() => (activeFn() ? "primary" : "tertiary"))
    .paddingHorizontal(8)
    .paddingVertical(3)
    .cornerRadius(6)
    .background(() => (activeFn() ? WASH_STRONG : null))
    .hoverBackground(WASH_STRONG)
    .onTap(onTap);
}

function viewToggle() {
  viewMode();
  return HStack({ spacing: 4 }, [
    modeChip("Board", () => viewMode() === "board", () => setViewMode("board")),
    modeChip("List", () => viewMode() === "list", () => setViewMode("list")),
  ]);
}

function scopeToggle() {
  scopeMode();
  return HStack({ spacing: 4 }, [
    modeChip("Host", () => scopeMode() === "host", () => setScopeMode("host")),
    modeChip("Focus", () => scopeMode() === "focus", () => setScopeMode("focus")),
    modeChip("Assigned", () => scopeMode() === "assigned", () => setScopeMode("assigned")),
  ]);
}

function sectionLabel(title, countFn) {
  return HStack({ spacing: 6 }, [
    Text(() => (typeof title === "function" ? title() : title))
      .font(10)
      .weight("semibold")
      .color("tertiary"),
    Spacer(),
    Text(() => String(countFn()))
      .font(10)
      .monospaced()
      .color("tertiary"),
  ]).paddingHorizontal(10);
}

// Flat row like panel-todo / Files: idle transparent, hover wash, 3pt rail.
function beadRow(s, w) {
  return HStack({ spacing: 0 }, [
    RoundedRectangle({ width: 3, cornerRadius: 1 })
      .fill(() => chipTint(s()))
      .frame({ height: 28 }),
    VStack({ spacing: 1 }, [
      Text(() => beadTitle(s()))
        .font(13)
        .lineLimit(1)
        .truncation("tail")
        .marquee()
        .color("primary"),
      HStack({ spacing: 6 }, [
        Text(() => beadStatusName(s()))
          .font(10)
          .monospaced()
          .color("tertiary")
          .lineLimit(1),
        Text(() => (focusTagOf(s()) ? FOCUS_MARK : ""))
          .font(10)
          .color("tertiary"),
      ]),
    ]).paddingLeading(8),
    Spacer({ minLength: 0 }),
  ])
    .paddingHorizontal(10)
    .paddingVertical(6)
    .cornerRadius(8)
    .hoverBackground(WASH)
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
    .cornerRadius(8)
    .hoverBackground(WASH)
    .frame({ maxWidth: "infinity" })
    .onTap(() => cmux("surface.focus", { surface_id: tabFocusId(t()) }));
}

function kanbanColumn(section, w) {
  return VStack({ spacing: 2 }, [
    sectionLabel(
      () => columnTitle(section().col),
      () => (section().items ?? []).length,
    ),
    ForEach(
      {
        items: () => (section().items ?? []).slice(0, MAX_BEADS),
        key: (s) => s.key ?? s.value,
      },
      (s) => beadRow(s, w),
    ),
  ]);
}

function beadsBoard(w) {
  viewMode();
  scopeMode();
  const emptyHint = () => {
    if (scopedBeads(w()).length > 0) return "";
    if (beadStatuses(w()).length === 0) {
      return "Run cmux-beads watch to load the board.";
    }
    if (scopeMode() === "focus") {
      return "No beads for this focus. Assign in the TUI (A) or switch to Host.";
    }
    if (scopeMode() === "assigned") {
      return "No pane-assigned beads yet. Assign in the TUI (A), then watch.";
    }
    return "";
  };
  return VStack({ spacing: 6 }, [
    Text(emptyHint)
      .font(11)
      .color("tertiary")
      .paddingHorizontal(10)
      .lineLimit(3),
    ForEach(
      {
        items: () =>
          viewMode() === "list" ? listSections(w()) : kanbanSections(w()),
        key: (s) => (viewMode() === "list" ? "list:" : "board:") + s.id,
      },
      (section) => kanbanColumn(section, w),
    ),
  ]);
}

function surfaces(w) {
  return VStack({ spacing: 2 }, [
    sectionLabel(
      "SURFACES",
      () => (w().tabs ?? []).length,
    ),
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
  return sectionLabel("HOST", () => liveWorkspaces().length);
}

function selectedHeader() {
  return VStack({ spacing: 2 }, [
    HStack({ spacing: 8 }, [
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
      .paddingVertical(() => (data.selectedTitle() ? 6 : 0))
      .cornerRadius(8)
      .background(() => (data.selectedTitle() ? WASH_FAINT : null))
      .hoverBackground(() => (data.selectedTitle() ? WASH : null))
      .frame({ maxWidth: "infinity" }),
    ForEach(
      {
        items: selectedWorkspaces,
        key: (w) => "focus:" + w.id,
      },
      (w) =>
        Text(() => {
          const label = focusLabel(w());
          return label ? "Focus · " + label : "";
        })
          .font(10)
          .color("tertiary")
          .paddingHorizontal(10)
          .lineLimit(1),
    ),
  ]);
}

sidebar(
  () =>
    VStack({ spacing: 8 }, [
      HStack({ spacing: 6 }, [
        Text("Beads").font(14).weight("semibold").color("primary"),
        Spacer(),
        Text(() => (beadCount() ? String(beadCount()) : ""))
          .font(11)
          .color("tertiary"),
      ]).paddingHorizontal(10),
      HStack({ spacing: 4 }, [viewToggle(), Spacer(), scopeToggle()]).paddingHorizontal(
        10,
      ),
      selectedHeader(),
      ForEach(
        {
          items: selectedWorkspaces,
          key: (w) => "board:" + w.id,
        },
        (w) => beadsBoard(w),
      ),
      Divider(),
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
      Text("Status moves: cmux-beads update. Updates after sync or watch.")
        .font(11)
        .color("tertiary")
        .paddingHorizontal(10)
        .lineLimit(2),
      Spacer(),
    ]).paddingHorizontal(6),
  { surface: "glass" },
);
