import GuavaUI

struct Project: Identifiable, Hashable {
    let id: String
    let name: String
    let kind: String
    let updated: String
    let progress: Double
}

/// The sample data the screens share; edits stay for the session.
struct MobileModel {
    var projects = [
        Project(id: "crystal", name: "Crystal Rush", kind: "Game", updated: "2 min ago", progress: 0.72),
        Project(id: "harbor", name: "Harbor Night", kind: "Animation", updated: "1 h ago", progress: 0.4),
        Project(id: "trailer", name: "Launch Trailer", kind: "Video", updated: "Yesterday", progress: 0.9),
        Project(id: "guavaui", name: "GuavaUI", kind: "Code", updated: "Mon", progress: 0.55),
    ]
    var notifications = true
    var syncOverCellular = false
    var reduceMotion = false
    var appearance = "System"
}

extension Binding {
    /// A binding to one field of the bound value.
    func field<Field>(_ path: WritableKeyPath<Value, Field>) -> Binding<Field> {
        Binding<Field>(get: { wrappedValue[keyPath: path] }, set: { wrappedValue[keyPath: path] = $0 })
    }
}
