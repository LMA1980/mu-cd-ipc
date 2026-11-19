---
name: mu-cd-ipc
description: "Implement zero-copy framing and Apache Fory RPC contracts for MuConDe continuous delivery, A/B root switching, and integration test progress reporting."
---

# `mu-cd-ipc` Continuous Delivery Wire Contracts

## 1. Zero-Copy Framing & Apache Fory Control Plane

- Use `framing` (`zerocopy`) for fixed-header transport frames and `control` (`fory`) for schema-evolvable deployment RPCs between `mu-cd-controller` and `mu-cd-agent`.
- Include structured integration-test progress events (`IntegrationTestProgress`) and canary bake-window status payloads so `mu-cd-controller` and `MuConsole` track real-time rollout state.
