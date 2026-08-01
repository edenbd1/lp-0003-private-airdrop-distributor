// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Bridge exposed to QML as `bridge`. It builds a recipient's claim arguments by
// shelling out to the local `airdrop` CLI (crates/airdrop-cli), which reuses the
// same airdrop-core primitives the on-chain program verifies. It does not hold
// keys itself; the recipient points it at their distribution directory.

#pragma once

#include <QObject>
#include <QString>

class ClaimBridge : public QObject {
    Q_OBJECT
public:
    explicit ClaimBridge(QObject* parent = nullptr);

    // Build the SPEL claim arguments for one recipient of a distribution.
    // Returns the human-readable summary (nullifier, marker seed, allocation),
    // or an error string prefixed with "error:". Writes the args to `outPath`.
    Q_INVOKABLE QString buildClaimArgs(const QString& distDir, int index,
                                       const QString& outPath);

    // Path to the `airdrop` binary, overridable from QML for non-default installs.
    Q_INVOKABLE void setCliPath(const QString& path);

private:
    QString m_cli = QStringLiteral("airdrop");
};
