---
name: actantdb-graphify
description: Query, navigate, and update the repository knowledge graph to understand architecture and maintain context.
---
# ActantDB Graphify Skill

Use this skill when you need to understand dependencies, query codebase concepts, trace relationship paths, or update the repository's knowledge graph.

## Instructions

1. **Concept Explanations**:
   To explain a focused concept using the knowledge graph:
   ```bash
   graphify explain "<concept>"
   ```
2. **Relationship Paths**:
   To trace path relationships between component A and B:
   ```bash
   graphify path "<A>" "<B>"
   ```
3. **General Querying**:
   If the knowledge graph json `graphify-out/graph.json` exists, query codebase questions:
   ```bash
   graphify query "<question>"
   ```
4. **Wiki Navigation**:
   If `graphify-out/wiki/index.md` exists, use it for broad codebase navigation instead of manually browsing raw sources.
5. **Update Knowledge Graph**:
   After making modifications to the codebase, run:
   ```bash
   graphify update .
   ```
   This is an AST-only update and doesn't incur API costs.
