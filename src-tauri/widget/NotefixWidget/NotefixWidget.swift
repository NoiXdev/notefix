import WidgetKit
import SwiftUI

struct Item: Codable, Identifiable, Hashable { let id: String; let title: String }

struct Snapshot: Codable {
    var context: String = "Notefix"
    var count: Int = 0
    var pinned: [Item] = []
    var recent: [Item] = []
}

struct NotefixEntry: TimelineEntry { let date: Date; let snap: Snapshot }

struct Provider: TimelineProvider {
    func placeholder(in context: Context) -> NotefixEntry { NotefixEntry(date: Date(), snap: Snapshot()) }

    func getSnapshot(in context: Context, completion: @escaping (NotefixEntry) -> Void) {
        completion(NotefixEntry(date: Date(), snap: Self.read()))
    }

    func getTimeline(in context: Context, completion: @escaping (Timeline<NotefixEntry>) -> Void) {
        let entry = NotefixEntry(date: Date(), snap: Self.read())
        completion(Timeline(entries: [entry], policy: .after(Date().addingTimeInterval(900))))
    }

    static func read() -> Snapshot {
        guard let dir = FileManager.default.containerURL(
                forSecurityApplicationGroupIdentifier: "group.dev.noix.notefix"),
              let data = try? Data(contentsOf: dir.appendingPathComponent("widget.json")),
              let snap = try? JSONDecoder().decode(Snapshot.self, from: data)
        else { return Snapshot() }
        return snap
    }
}

private func noteURL(_ id: String) -> URL { URL(string: "notefix://note/\(id)")! }
private let newURL = URL(string: "notefix://new")!

struct Header: View {
    let snap: Snapshot
    var body: some View {
        HStack {
            VStack(alignment: .leading, spacing: 0) {
                Text("Notefix").font(.headline)
                Text(snap.context).font(.caption2).foregroundStyle(.secondary).lineLimit(1)
            }
            Spacer()
            Link(destination: newURL) {
                Image(systemName: "plus.circle.fill").font(.title2).foregroundStyle(.tint)
            }
        }
    }
}

struct Row: View {
    let item: Item
    var body: some View {
        Link(destination: noteURL(item.id)) {
            HStack(spacing: 6) {
                Image(systemName: "doc.text").font(.caption2).foregroundStyle(.secondary)
                Text(item.title).font(.caption).lineLimit(1)
                Spacer(minLength: 0)
            }
        }
    }
}

struct SmallView: View {
    let snap: Snapshot
    var body: some View {
        VStack(spacing: 6) {
            Spacer()
            Image(systemName: "plus.circle.fill").font(.system(size: 40)).foregroundStyle(.tint)
            Text("Neue Notiz").font(.headline)
            Spacer()
            Text("\(snap.context) · \(snap.count)").font(.caption2).foregroundStyle(.secondary).lineLimit(1)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .widgetURL(newURL)
    }
}

struct MediumView: View {
    let snap: Snapshot
    var rows: [Item] {
        var seen = Set<String>()
        var out: [Item] = []
        for i in snap.pinned + snap.recent where !seen.contains(i.id) {
            seen.insert(i.id)
            out.append(i)
        }
        return Array(out.prefix(3))
    }
    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Header(snap: snap)
            if rows.isEmpty {
                Spacer()
                Text("Keine Notizen").font(.caption).foregroundStyle(.secondary)
                Spacer()
            } else {
                ForEach(rows) { Row(item: $0) }
                Spacer(minLength: 0)
            }
        }
    }
}

struct LargeView: View {
    let snap: Snapshot
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Header(snap: snap)
            if snap.pinned.isEmpty && snap.recent.isEmpty {
                Spacer()
                Text("Keine Notizen").font(.caption).foregroundStyle(.secondary)
                Spacer()
            } else {
                if !snap.pinned.isEmpty {
                    Text("Angepinnt").font(.caption2).foregroundStyle(.secondary)
                    ForEach(Array(snap.pinned.prefix(4))) { Row(item: $0) }
                }
                if !snap.recent.isEmpty {
                    Text("Zuletzt").font(.caption2).foregroundStyle(.secondary).padding(.top, 2)
                    ForEach(Array(snap.recent.prefix(6))) { Row(item: $0) }
                }
                Spacer(minLength: 0)
            }
        }
    }
}

struct NotefixWidgetEntryView: View {
    @Environment(\.widgetFamily) var family
    var entry: Provider.Entry
    var body: some View {
        Group {
            switch family {
            case .systemSmall: SmallView(snap: entry.snap)
            case .systemLarge: LargeView(snap: entry.snap)
            default: MediumView(snap: entry.snap)
            }
        }
        .containerBackground(.fill.tertiary, for: .widget)
    }
}

struct NotefixWidget: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: "NotefixWidget", provider: Provider()) { entry in
            NotefixWidgetEntryView(entry: entry)
        }
        .configurationDisplayName("Notefix")
        .description("Neue Notiz, angepinnte und letzte Notizen.")
        .supportedFamilies([.systemSmall, .systemMedium, .systemLarge])
    }
}

@main
struct NotefixWidgetBundle: WidgetBundle {
    var body: some Widget { NotefixWidget() }
}
