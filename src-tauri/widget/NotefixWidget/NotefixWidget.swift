import WidgetKit
import SwiftUI

struct HelloEntry: TimelineEntry {
    let date: Date
    let text: String
}

struct Provider: TimelineProvider {
    func placeholder(in context: Context) -> HelloEntry { HelloEntry(date: Date(), text: "Notefix") }

    func getSnapshot(in context: Context, completion: @escaping (HelloEntry) -> Void) {
        completion(HelloEntry(date: Date(), text: Self.readHello()))
    }

    func getTimeline(in context: Context, completion: @escaping (Timeline<HelloEntry>) -> Void) {
        let entry = HelloEntry(date: Date(), text: Self.readHello())
        completion(Timeline(entries: [entry], policy: .after(Date().addingTimeInterval(60))))
    }

    static func readHello() -> String {
        guard let dir = FileManager.default.containerURL(
            forSecurityApplicationGroupIdentifier: "group.dev.noidee.notefix") else { return "Hello Notefix" }
        let url = dir.appendingPathComponent("widget.json")
        guard let data = try? Data(contentsOf: url),
              let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let hello = obj["hello"] as? String else { return "Hello Notefix" }
        return hello
    }
}

struct NotefixWidgetEntryView: View {
    var entry: Provider.Entry
    var body: some View {
        VStack(spacing: 4) {
            Text("Notefix").font(.caption).foregroundStyle(.secondary)
            Text(entry.text).font(.headline).multilineTextAlignment(.center)
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
        .description("Notefix spike widget.")
        .supportedFamilies([.systemSmall])
    }
}

@main
struct NotefixWidgetBundle: WidgetBundle {
    var body: some Widget { NotefixWidget() }
}
