internal import SwiftSyntax
internal import SwiftSyntaxBuilder
internal import SwiftSyntaxMacros

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
        let rewrite = FieldReads(states: names, shadowed: locals.names)
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
    init(states: Set<String>, shadowed: Set<String>) {
        self.states = states
        self.shadowed = shadowed
        super.init(viewMode: .sourceAccurate)
    }

    override func visit(_ node: MemberAccessExprSyntax) -> ExprSyntax {
        if let (state, fields) = path(ExprSyntax(node)), !fields.isEmpty {
            let field = fields.joined(separator: ".")
            return ExprSyntax(stringLiteral: "self._\(state)[tracking: \\.\(field), name: \"\(state).\(field)\"]")
                .with(\.leadingTrivia, node.leadingTrivia).with(\.trailingTrivia, node.trailingTrivia)
        }
        return super.visit(node)
    }

    override func visit(_ node: FunctionCallExprSyntax) -> ExprSyntax {
        // A method is not a key path. Rewrite only its receiver, preserving
        // mutations such as state.flag.toggle() and ordinary function calls.
        guard let member = node.calledExpression.as(MemberAccessExprSyntax.self), let base = member.base else {
            return super.visit(node)
        }
        var result = node
        result.calledExpression = ExprSyntax(member.with(\.base, rewrite(base).as(ExprSyntax.self)))
        result.arguments = rewrite(node.arguments).as(LabeledExprListSyntax.self) ?? node.arguments
        if let trailing = node.trailingClosure { result.trailingClosure = rewrite(trailing).as(ClosureExprSyntax.self) }
        result.additionalTrailingClosures = rewrite(node.additionalTrailingClosures).as(MultipleTrailingClosureElementListSyntax.self) ?? node.additionalTrailingClosures
        return ExprSyntax(result)
    }

    private func path(_ expression: ExprSyntax) -> (String, [String])? {
        if let reference = expression.as(DeclReferenceExprSyntax.self), states.contains(reference.baseName.text),
           !shadowed.contains(reference.baseName.text) { return (reference.baseName.text, []) }
        guard let member = expression.as(MemberAccessExprSyntax.self), let base = member.base else { return nil }
        let name = member.declName.baseName.text
        if base.trimmedDescription == "self", states.contains(name) { return (name, []) }
        guard let (state, fields) = path(base) else { return nil }
        return (state, fields + [name])
    }
}
