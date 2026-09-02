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
// Restricted Swift subset (cmux docs/custom-sidebars.md). Native right-sidebar
// rendering, not an iframe and not a PTY stuffed in a pane. Root is a view
// expression, not a struct. Bind only live cmux context: workspaces, tabs,
// statuses, agents, progress, git, color. No invented team or fake rows.
// The Beads board is the product. Host workspaces are the switcher.
// Bead rows appear after `cmux-beads sync` / `watch` writes bead:<id> keys.
// Chrome uses Ghostty/cmux theme tokens so dark/light follow the host.

func hasText(_ value) -> Bool {
  return value != nil && value != ""
}

func hasStatuses(_ w) -> Bool {
  return w.statuses != nil && w.statuses.count > 0
}

func hasAgents(_ w) -> Bool {
  return w.agents != nil && w.agents.count > 0
}

func hasTabs(_ w) -> Bool {
  return w.tabs != nil && w.tabs.count > 0
}

func statusTint(_ s) -> String {
  if hasText(s.color) { return s.color }
  return "accent"
}

func statusLabel(_ s) -> String {
  if hasText(s.value) { return s.value }
  if hasText(s.key) { return s.key }
  return ""
}

func isBeadStatus(_ s) -> Bool {
  return hasText(s.key) && s.key.hasPrefix("bead:")
}

func agentTint(_ a) -> String {
  if a.status == "working" { return "accent" }
  if a.status == "needs_input" { return "red" }
  if a.status == "ended" { return "tertiary" }
  return "secondary"
}

func agentIcon(_ a) -> String {
  if a.status == "working" { return "hammer.fill" }
  if a.status == "needs_input" { return "exclamationmark.triangle.fill" }
  if a.status == "ended" { return "checkmark.circle" }
  return "pause.circle"
}

func tabFocusId(_ t) -> String {
  if hasText(t.surfaceId) { return t.surfaceId }
  return t.id
}

func beadsStatusChip(_ s) -> some View {
  HStack(spacing: 0) {
    RoundedRectangle(cornerRadius: 2)
      .frame(width: 3, height: 36)
      .foregroundColor(statusTint(s))
    Text(statusLabel(s))
      .font(.system(size: 13))
      .fontWeight(.semibold)
      .foregroundColor("primary")
      .lineLimit(1)
      .padding(.leading, 9)
    Spacer()
  }
  .padding(.horizontal, 10)
  .padding(.vertical, 7)
  .background {
    RoundedRectangle(cornerRadius: 10)
      .foregroundColor("#7f7f7f14")
  }
}

func beadsAgentChip(_ a) -> some View {
  HStack(spacing: 4) {
    Image(systemName: agentIcon(a))
      .font(.system(size: 8))
      .foregroundColor(agentTint(a))
      .symbolRenderingMode(.hierarchical)
    Text(a.name)
      .font(.system(size: 10))
      .foregroundColor(agentTint(a))
      .lineLimit(1)
  }
}

func beadsWorkspaceRow(_ w) -> some View {
  Button(action: { cmux("workspace.select", workspace_id: w.id) }) {
    HStack(spacing: 8) {
      Image(systemName: "line.3.horizontal")
        .font(.system(size: 9))
        .foregroundColor("tertiary")
      Circle()
        .frame(width: 7, height: 7)
        .foregroundColor(w.selected ? "accent" : "tertiary")
      Text(w.title)
        .font(.system(size: 13))
        .foregroundColor(w.selected ? "primary" : "secondary")
        .lineLimit(1)
      Spacer()
      if w.pinned && !(w.unread > 0) {
        Image(systemName: "pin.fill")
          .font(.system(size: 8))
          .foregroundColor("tertiary")
      }
      if w.unread > 0 {
        Text("\(w.unread)")
          .font(.system(size: 10))
          .fontWeight(.bold)
          .foregroundColor("white")
          .padding(.horizontal, 5)
          .padding(.vertical, 1)
          .background {
            Capsule().foregroundColor("#E4573D")
          }
      }
    }
    .padding(.vertical, 6)
    .padding(.horizontal, 10)
    .background {
      RoundedRectangle(cornerRadius: 8)
        .foregroundColor(w.selected ? "#7f7f7f3d" : "#7f7f7f00")
    }
  }
  .contextMenu {
    Button("Select") { cmux("workspace.select", workspace_id: w.id) }
    Button(w.pinned ? "Unpin" : "Pin") {
      cmux("workspace.action", action: w.pinned ? "unpin" : "pin", workspace_id: w.id)
    }
    Button(w.unread > 0 ? "Mark as Read" : "Mark as Unread") {
      cmux("workspace.action", action: w.unread > 0 ? "mark_read" : "mark_unread", workspace_id: w.id)
    }
    Button("Move Up") { cmux("workspace.action", action: "move_up", workspace_id: w.id) }
    Button("Move Down") { cmux("workspace.action", action: "move_down", workspace_id: w.id) }
    Button("Move to Top") { cmux("workspace.action", action: "move_top", workspace_id: w.id) }
  }
}

func beadsTabRow(_ t) -> some View {
  Button(action: { cmux("surface.focus", surface_id: tabFocusId(t)) }) {
    HStack(spacing: 6) {
      Image(systemName: t.focused ? "dot.circle.fill" : "terminal")
        .font(.system(size: 10))
        .foregroundColor(t.focused ? "accent" : "secondary")
        .frame(width: 14)
      Text(t.title)
        .font(.system(size: 12))
        .foregroundColor(t.focused ? "primary" : "secondary")
        .lineLimit(1)
      Spacer()
    }
    .padding(.vertical, 5)
    .padding(.horizontal, 10)
    .background {
      RoundedRectangle(cornerRadius: 7)
        .foregroundColor("#7f7f7f14")
    }
  }
}

func beadsBoard(_ w) -> some View {
  VStack(alignment: .leading, spacing: 6) {
    if hasStatuses(w) {
      ForEach(w.statuses.filter { isBeadStatus($0) }.prefix(24)) { s in
        beadsStatusChip(s)
      }
    }
    if !hasStatuses(w) {
      Text("Run cmux-beads watch to load the Beads board.")
        .font(.caption)
        .foregroundColor("tertiary")
    }
  }
}

func hostSurfaces(_ w) -> some View {
  VStack(alignment: .leading, spacing: 6) {
    if hasAgents(w) {
      Text("Live agents")
        .font(.system(size: 10))
        .fontWeight(.semibold)
        .foregroundColor("tertiary")
      ForEach(w.agents.prefix(8)) { a in
        beadsAgentChip(a)
      }
    }
    if hasTabs(w) {
      Text("SURFACES")
        .font(.system(size: 10))
        .fontWeight(.semibold)
        .foregroundColor("tertiary")
      ForEach(w.tabs.prefix(12)) { t in
        beadsTabRow(t)
      }
    }
  }
}

ScrollView {
  VStack(alignment: .leading, spacing: 10) {
    HStack(spacing: 8) {
      Image(systemName: "circle.grid.3x3")
        .foregroundColor("accent")
        .symbolRenderingMode(.hierarchical)
      Text("Beads")
        .font(.system(size: 14))
        .fontWeight(.semibold)
        .foregroundColor("primary")
      Spacer()
      Text("\(workspaceCount)")
        .font(.system(size: 11, design: .monospaced))
        .foregroundColor("tertiary")
    }
    if hasText(selectedTitle) {
      Text(selectedTitle)
        .font(.system(size: 13))
        .fontWeight(.semibold)
        .foregroundColor("primary")
        .lineLimit(1)
        .padding(.horizontal, 10)
        .padding(.vertical, 8)
        .background {
          RoundedRectangle(cornerRadius: 10)
            .foregroundColor("#7f7f7f24")
        }
    }

    Divider()

    ForEach(workspaces.filter { $0.selected }.prefix(1)) { w in
      beadsBoard(w)
    }

    Divider()

    VStack(alignment: .leading, spacing: 6) {
      HStack {
        Text("HOST")
          .font(.system(size: 10))
          .fontWeight(.semibold)
          .foregroundColor("tertiary")
        Spacer()
        Text("\(workspaceCount)")
          .font(.system(size: 10, design: .monospaced))
          .foregroundColor("tertiary")
      }
      if workspaces.count == 0 {
        Text("No live host workspace")
          .font(.caption)
          .foregroundColor("tertiary")
      }
      if workspaces.count > 0 {
        Reorderable(workspaces.prefix(40), move: "workspace.reorder") { w in
          beadsWorkspaceRow(w)
        }
      }
    }

    ForEach(workspaces.filter { $0.selected }.prefix(1)) { w in
      hostSurfaces(w)
    }

    Divider()

    Text("Beads board updates after cmux-beads sync or watch.")
      .font(.caption)
      .foregroundColor("tertiary")
      .lineLimit(3)
  }
  .padding(12)
}
