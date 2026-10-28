internal import SwiftSyntax
internal import SwiftSyntaxBuilder
internal import SwiftSyntaxMacros
internal import SwiftDiagnostics

struct TrackStateReadsMacro: MemberMacro, ExtensionMacro {
    static func expansion(of node: AttributeSyntax, providingMembersOf declaration: some DeclGroupSyntax,
                          conformingTo protocols: [TypeSyntax], in context: some MacroExpansionContext) throws -> [DeclSyntax] {
        guard declaration.is(StructDeclSyntax.self) else { throw Failure("@TrackStateReads requires a view struct") }
        let properties = declaration.memberBlock.members.compactMap { $0.decl.as(VariableDeclSyntax.self) }
        let names = Set(properties.filter { property in
            property.attributes.contains { attribute in
                attribute.as(AttributeSyntax.self)?.attributeName.trimmedDescription.split(separator: ".").last == "State"
            }
        }.flatMap { $0.bindings.compactMap { $0.pattern.as(IdentifierPatternSyntax.self)?.identifier.text } })
        guard let body = properties.first(where: { $0.bindings.first?.pattern.trimmedDescription == "body" }),
              let accessor = body.bindings.first?.accessorBlock else { throw Failure("@TrackStateReads requires a body getter") }
        let statements: CodeBlockItemListSyntax
        switch accessor.accessors {
        case .getter(let code): statements = code
        case .accessors(let getters):
            guard let getter = getters.first(where: { $0.accessorSpecifier.text == "get" }), let block = getter.body else {
                throw Failure("@TrackStateReads requires a body getter")
            }
            statements = block.statements
        }
        let locals = LocalNames(viewMode: .sourceAccurate)
        locals.walk(statements)
        let rewrite = FieldReads(states: names, shadowed: locals.names, context: context)
        let result = rewrite.rewrite(statements)
        let modifiers = declaration.modifiers
        let access = modifiers.contains(where: { ["public", "open"].contains($0.name.text) }) ? "public " : ""
        return [DeclSyntax(stringLiteral: "@ViewBuilder \(access)var _coveTrackedBody: some View {\n\(result)\n}")]
    }

    static func expansion(of node: AttributeSyntax, attachedTo declaration: some DeclGroupSyntax,
                          providingExtensionsOf type: some TypeSyntaxProtocol, conformingTo protocols: [TypeSyntax],
                          in context: some MacroExpansionContext) throws -> [ExtensionDeclSyntax] {
        [try ExtensionDeclSyntax("extension \(type.trimmed): CoveUIComposeCore._StateReadTrackedView {}")]
    }

    private struct Failure: Error, CustomStringConvertible {
        let description: String
        init(_ description: String) { self.description = description }
    }
}

/// Shadowing anywhere in the body conservatively disables unqualified rewrites
/// of that name. `self.state.field` remains unambiguous in every lexical scope.
private final class LocalNames: SyntaxVisitor {
    var names: Set<String> = []
    override func visit(_ node: IdentifierPatternSyntax) -> SyntaxVisitorContinueKind {
        names.insert(node.identifier.text)
        return .visitChildren
    }
    override func visit(_ node: ClosureParameterSyntax) -> SyntaxVisitorContinueKind {
        names.insert((node.secondName ?? node.firstName).text)
        return .visitChildren
    }
    override func visit(_ node: FunctionParameterSyntax) -> SyntaxVisitorContinueKind {
        names.insert((node.secondName ?? node.firstName).text)
        return .visitChildren
    }
    override func visit(_ node: ClosureCaptureSyntax) -> SyntaxVisitorContinueKind {
        names.insert(node.name.text)
        return .visitChildren
    }
    override func visit(_ node: CatchClauseSyntax) -> SyntaxVisitorContinueKind {
        if node.catchItems.isEmpty { names.insert("error") }
        return .visitChildren
    }
    override func visit(_ node: ClosureShorthandParameterSyntax) -> SyntaxVisitorContinueKind {
        names.insert(node.name.text)
        return .visitChildren
    }
}

private final class FieldReads: SyntaxRewriter {
    let states: Set<String>
    let shadowed: Set<String>
    let context: MacroExpansionContext

    init(states: Set<String>, shadowed: Set<String>, context: MacroExpansionContext) {
        self.states = states
        self.shadowed = shadowed
        self.context = context
        super.init(viewMode: .sourceAccurate)
    }

    /// Warn when a `@State` property is referenced as a whole value (not through a
    /// field member), which defeats precise invalidation. The binding initializer
    /// `if let x = state.field` is still rewritten to a field read, so this only
    /// fires for genuinely whole reads such as `let snapshot = state`. It is
    /// suppressed where a whole read is unavoidable/uninformative: as the base of
    /// a subscript (e.g. `state[signal: …]`) or as a condition (`if flag`).
    override func visit(_ node: DeclReferenceExprSyntax) -> ExprSyntax {
        let name = node.baseName.text
        if states.contains(name), !shadowed.contains(name) {
            let parent = node.parent
            let isSubscriptBase = parent?.is(SubscriptExprSyntax.self) == true
            let isCondition = parent?.is(ConditionElementSyntax.self) == true
            if !isSubscriptBase, !isCondition {
                context.diagnose(Diagnostic(node: Syntax(node), message: WholeValueReadMessage(state: name)))
            }
        }
        return super.visit(node)
    }

    override func visit(_ node: MemberAccessExprSyntax) -> ExprSyntax {
        // A member access that is the receiver of a function call (e.g.
        // `model.name.uppercased()` or `model.name.append("x")`) is a method
        // invocation, not a field read: only the base (`model.name`) should be
        // rewritten to a tracking read, and the method call kept intact.
        if node.parent?.is(FunctionCallExprSyntax.self) != true,
           let (state, p) = statePath(ExprSyntax(node)), !p.isEmpty {
            let literal = "self._\(state)[tracking: \\.\(p), name: \"\(state).\(p)\"]"
            return ExprSyntax(stringLiteral: literal)
                .with(\.leadingTrivia, node.leadingTrivia)
                .with(\.trailingTrivia, node.trailingTrivia)
        }
        return super.visit(node)
    }

    /// Resolve a `state.field` / `state.a?.b` / `self.state.field` expression to
    /// `(stateName, keyPathString)`. Returns `nil` when the expression does not
    /// read a tracked `@State`. Key paths must place `?` at each optional
    /// intermediate (`\state.a?.b`), so the optional-chaining markers are
    /// preserved explicitly.
    private func statePath(_ expression: ExprSyntax) -> (String, String)? {
        let expr: ExprSyntax
        if let oc = expression.as(OptionalChainingExprSyntax.self) { expr = oc.expression }
        else if let fu = expression.as(ForceUnwrapExprSyntax.self) { expr = fu.expression }
        else { expr = expression }

        if let ref = expr.as(DeclReferenceExprSyntax.self) {
            if states.contains(ref.baseName.text), !shadowed.contains(ref.baseName.text) {
                return (ref.baseName.text, "")
            }
            return nil
        }
        guard let member = expr.as(MemberAccessExprSyntax.self), let base = member.base else { return nil }
        let name = member.declName.baseName.text
        if base.trimmedDescription == "self", states.contains(name) {
            return (name, "")
        }
        guard let (state, basePath) = statePath(base) else { return nil }
        let baseWasOptional = base.is(OptionalChainingExprSyntax.self) || base.is(ForceUnwrapExprSyntax.self)
        let full: String
        if basePath.isEmpty {
            full = name
        } else if baseWasOptional {
            full = markLastOptional(basePath) + ".\(name)"
        } else {
            full = "\(basePath).\(name)"
        }
        return (state, full)
    }

    /// Append `?` to the final component of a key-path segment so that
    /// `\a.b?.c` (not `\a.b.c?`) is produced for mid-chain optional chaining.
    private func markLastOptional(_ path: String) -> String {
        guard let lastDot = path.lastIndex(of: ".") else { return path + "?" }
        let head = String(path[..<lastDot])
        let tail = String(path[path.index(after: lastDot)...])
        return "\(head).\(tail)?"
    }
}

private struct WholeValueReadMessage: DiagnosticMessage {
    let state: String
    var message: String {
        "@State '\(state)' is read as a whole value; read '\(state).field' for precise invalidation"
    }
    var severity: DiagnosticSeverity { .warning }
    var diagnosticID: MessageID { .init(domain: "CoveUIReadMacros", id: "whole-value-read") }
}
