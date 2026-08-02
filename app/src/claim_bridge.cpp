// SPDX-License-Identifier: MIT OR Apache-2.0
#include "claim_bridge.h"

#include <QFileInfo>
#include <QProcess>
#include <QStringList>

#include <dlfcn.h>

namespace {

// A GUI-launched app inherits the login shell's PATH, which usually does not
// contain the `airdrop` CLI we ship next to the plugin, so a hardcoded "airdrop"
// would load and then fail on the first click — the worst of both.
//
// dladdr gives us the path of the binary this code lives in, so the sibling is
// resolvable at runtime on both macOS and Linux without hardcoding anything.
QString resolveCli() {
    Dl_info info{};
    if (dladdr(reinterpret_cast<const void*>(&resolveCli), &info) && info.dli_fname) {
        const QFileInfo self(QString::fromUtf8(info.dli_fname));
        const QString sibling = self.absolutePath() + QStringLiteral("/airdrop");
        if (QFileInfo(sibling).isExecutable()) {
            return sibling;
        }
    }
    // Developer builds run the plugin out of a build tree with no CLI beside
    // it; there, PATH is the right answer.
    return QStringLiteral("airdrop");
}

}  // namespace

ClaimBridge::ClaimBridge(QObject* parent) : QObject(parent), m_cli(resolveCli()) {}

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
