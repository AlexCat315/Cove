/// Infer @State field dependencies from a view's body at compile time.
///
/// ```swift
/// @TrackStateReads
/// struct ProfileView: View {
///     @State var profile = Profile()
///     var body: some View { Text(profile.name) }
/// }
/// ```
/// The macro tracks `profile.name`; changes to other Equatable fields do not
/// invalidate this scope. Whole-value reads and opaque fields remain conservative.
@attached(member, names: named(_coveTrackedBody))
@attached(extension, conformances: _StateReadTrackedView)
public macro TrackStateReads() = #externalMacro(module: "CoveUIReadMacros", type: "TrackStateReadsMacro")

/// Runtime entry point synthesized by `@TrackStateReads`.
public protocol _StateReadTrackedView: View {
    associatedtype TrackedBody: View
    @ViewBuilder var _coveTrackedBody: TrackedBody { get }
}
