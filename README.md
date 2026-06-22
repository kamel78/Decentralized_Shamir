# SafeTender: A Secure Decentralized E-Procurement Framework via Client-Side Threshold Cryptography

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Language: Rust](https://img.shields.io/badge/Language-Rust-orange.svg)](https://www.rust-lang.org/)
[![Wasm: Supported](https://img.shields.io/badge/Wasm-Supported-blueviolet.svg)](https://webassembly.org/)

This repository contains the official reference implementation for **SafeTender**, a decentralized, web-native e-procurement framework engineered to eliminate the *Chronological Paradox* inherent to legacy threshold-based electronic voting and bidding architectures. 

By integrating an asymmetric password-authenticated key exchange (aPAKE) protocol with an end-to-end client-side threshold secret sharing scheme, SafeTender removes the requirement for a trusted central dealer. It enforces absolute data confidentiality, public verifiability, and collusion resistance against a compromised backend cloud infrastructure.

---

## 1. Core Cryptographic Paradigm

Traditional threshold e-procurement architectures rely on a centralized *Trusted Dealer* or a pre-computed Shamir Secret Sharing (SSS) initialization phase. This creates a severe **Chronological Paradox**: a master decryption key ($\mathit{SK}$) must conceptually exist *prior* to the distribution of the global public key ($\mathit{PK}_{\mathsf{global}}$), presenting a catastrophic single point of failure (SPOF) during the submission window.

SafeTender completely resolves this paradox through an automated, server-mediated, decentralized distribution architecture operating directly inside client-side WebAssembly sandboxes:

### Registration Protocol
Each Commission Member $P_i$ registers via the **OPAQUE aPAKE** protocol. During initialization, the client browser locally generates a long-term asymmetric key pair $(\mathit{sk}_i^{\mathsf{comm}}, \mathit{pk}_i^{\mathsf{comm}})$.
* **Public Parameter Broadcast:** The public key $\mathit{pk}_i^{\mathsf{comm}}$ is uploaded openly to the server repository to enable multi-party encrypted routing.
* **Zero-Knowledge Private State Backup:** The private key $\mathit{sk}_i^{\mathsf{comm}}$ is symmetrically encrypted client-side using the derived OPAQUE export key $\mathit{K}_i^{\mathsf{exp}}$. The resulting ciphertext wrapper is backed up securely within $P_i$'s designated server partition, remaining fundamentally unreadable to the storage engine.

### Sub-Share Staging Protocol
When a procurement timeline is initialized, members compute local secret scalars $s_i$ as the constant terms of randomly constructed polynomials $f_i(x)$ of degree $t-1$:

$$f_i(x) = s_i + a_{i,1}x + a_{i,2}x^2 + \cdots + a_{i,t-1}x^{t-1} \pmod{q}$$

Rather than transmitting raw generated sub-shares $s_{i,j} = f_i(j) \pmod{q}$ over peer-to-peer network channels, the source node $P_i$ encrypts the sub-share using the target recipient's public key:

$$\tilde{\mathit{Sh}}_{i,j} = \mathsf{Enc}_{\mathit{pk}_j^{\mathsf{comm}}}(s_{i,j})$$

The resulting ciphertexts are deposited into $$P_j$$'s designated storage space on the server. Finally, each member $$P_j$$ authenticates via OPAQUE, recovers their encrypted private key 

$$\mathit{sk}_j^{\mathsf{comm}}$$ using $$\mathit{K}_j^{\mathsf{exp}}$$, 

and uses it locally within their browser sandbox to decrypt the deposited sub-shares $\tilde{\mathit{Sh}}_{i,j}$ sent by the other participants to generate 

$$\mathit{PK}_{\mathsf{global}} = \sum \mathit{PK}_i$$.

---

## 2. System Architecture

The project is structured as a multi-tier repository dividing client-side cryptographic logic, a WebAssembly frontend interface, and a centralized authentication and coordination server.

```text
.
├── LICENSE
├── README.md
├── client/                     # Native Client-Side Cryptographic Engine (Rust)
│   ├── Cargo.toml              # Compiles core primitives to WebAssembly (axclient)
│   ├── pkg/                    # Generated wasm-bindgen build artifacts
│   │   ├── axclient.js         # JavaScript glue code
│   │   └── axclient_bg.wasm    # Compiled Wasm binary
│   └── src/
│       └── lib.rs              # Client-side cryptographic operations
└── server/                     # Centralized Coordination & Backend Server
    ├── Cargo.toml              # Axum server configuration
    ├── tests/                  # Integration and infrastructure tests
    │   └── db_connection.rs    # Database connectivity verification
    ├── wasm_interface/         # Frontend Web Interface Layout
    │   ├── Cargo.toml          # Static asset interface layer
    │   ├── html/               # Interactive workflows (sharing, encryption, reconstruction)
    │   ├── css/ & webfonts/    # UI presentation and typography assets
    │   ├── scripts/            # Client side core interaction logic (core.js, files.js)
    │   └── src/lib.rs          # Web-assembly client frontend endpoint mapping
    ├── static/                 # Static Server Assets & Web Templates
    │   ├── emails/             # HTML mail forms (invitations for commissions & reconstruction)
    │   ├── html/               # Authentication & dashboard templates (login, register, dash)
    │   └── pkg/                # Mirrored WebAssembly runtime artifacts for server delivery
    └── src/                    # Backend Core Logic Architecture
        ├── main.rs             # Application server entry point
        ├── authorization_jwt/  # Stateless JSON Web Token validation micro-service
        ├── authentication_opaque/ # OPAQUE aPAKE Protocol Implementation
        │   ├── opaque_server.rs   # Server-side registration and login state machines
        │   └── cipher_suite.rs    # Cryptographic suite configurations
        ├── handlers/           # Route Controllers & Request Endpoints
        │   ├── auth_handlers.rs   # Session and enrollment request routing
        │   ├── shamir.rs          # Threshold cryptographic share transport triggers
        │   └── marche_handlers.rs # Procurement workflow operations
        └── entities/           # Database Domain Models & Persistence Layer
            ├── db.rs              # Database pool manager connection engine
            ├── users.rs           # Account identities data mappings
            ├── user_keys.rs       # Server-blind OPAQUE password file credentials
            └── commission_shares.rs # Encrypted decentralized secret pieces tracking
```

## 3. Cryptographic Trust Boundary

The architecture splits trust vertically to ensure maximum resistance against zero-day infrastructure exploits:

```text
+-------------------------------------------------------------------+
|               DATABASE TIER (PostgreSQL + SeaORM)                |
|      Houses Metadata, OPAQUE Server Keys, & Blinded Shares       |
+-------------------------------------------------------------------+
                              │
                              ▼
+-------------------------------------------------------------------+
|              UNTRUSTED SERVER CORE (Rust Axum Engine)            |
|     Acts strictly as a relational routing & scheduling state     |
+-------------------------------------------------------------------+
                              │
================== TLS 1.3 SECURE TRANSPORT CANAL ==================
                              │
CRITICAL CRYPTOGRAPHIC TRUST BOUNDARY - - - - - - - - - - - - - - - -
                              │
+-------------------------------------------------------------------+
|               CLIENT BROWSER RUNTIME (Wasm Sandbox)              |
|                                                                   |
|  [Commission Node Module]          [Public Bidder Interface]      |
|  - Local Polynomial Execution      - ECIES Payload Encryption     |
|  - Private Key SK Extraction       - secp256k1 Native Wrapping    |
|  - Local Decryption Processing     - Blind Sub-share Generation   |
+-------------------------------------------------------------------+
```
## 4. Building and Deployment

### Prerequisites
Ensure your build machine has the stable Rust toolchain and the WebAssembly target compiler installed:
```bash
rustup default stable
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
```

### Database InitializationSafeTender
```bash
uses SeaORM for asynchronous migration tracking.
```
Configure your target database connection string in your environment profile, then run the migration module:

```bash
export DATABASE_URL="postgres://username:password@localhost:5432/safetender_db"
cargo run --manifest-path storage/migration/Cargo.toml
```
### Client Cryptographic WebAssembly CompilationCompile the zero-knowledge mathematical libraries into optimized WebAssembly binaries:
```bash
cd client/
wasm-pack build --target web --release
```
### Backend Server DeploymentLaunch the primary Axum routing instance. 

By default, the server spins up a secure asynchronous interface listening on 127.0.0.1:8080:
```bash
cd ../server/
cargo run --release
```
### Security Evaluation MetricsServer-Blind Storage: 
The cloud server never handles plain scalars. All assets are wrapped in transit using public keys ($\mathit{pk}_j^{\mathsf{comm}}$) or encrypted at rest using high-entropy OPAQUE-derived export profiles ($\mathit{K}_i^{\mathsf{exp}}$).
Collusion Resistance: Any adversary compromising the backend hosting environment cannot reconstruct the master bidding keys or read submitted proposals, provided that fewer than $t$ commission members are compromised.Ephemeral Memory State Cleanup: Memory segments handling raw private keys ($\mathit{sk}_i^{\mathsf{comm}}$) or cleartext sub-shares utilize targeted memory-purging patterns to protect against local client-side inspection attacks.
