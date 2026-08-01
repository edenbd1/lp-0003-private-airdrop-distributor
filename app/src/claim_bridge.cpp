// SPDX-License-Identifier: MIT OR Apache-2.0
#include "claim_bridge.h"

#include <QProcess>
#include <QStringList>

ClaimBridge::ClaimBridge(QObject* parent) : QObject(parent) {}

void ClaimBridge::setCliPath(const QString& path) { m_cli = path; }

QString ClaimBridge::buildClaimArgs(const QString& distDir, int index,
                                    const QString& outPath) {
    QProcess proc;
    proc.start(m_cli, QStringList()
                          << QStringLiteral("claim-args")
                          << QStringLiteral("--dir") << distDir
                          << QStringLiteral("--index") << QString::number(index)
                          << QStringLiteral("--out") << outPath);
    if (!proc.waitForStarted(3000)) {
        return QStringLiteral("error: could not start '%1'. Set the CLI path.")
            .arg(m_cli);
    }
    proc.waitForFinished(15000);
    const QString out = QString::fromUtf8(proc.readAllStandardOutput());
    const QString err = QString::fromUtf8(proc.readAllStandardError());
    if (proc.exitCode() != 0) {
        return QStringLiteral("error: %1").arg(err.isEmpty() ? out : err);
    }
    return out.trimmed();
}
