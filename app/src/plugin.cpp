// SPDX-License-Identifier: MIT OR Apache-2.0
#include "plugin.h"
#include "claim_bridge.h"

#include <QQmlContext>
#include <QQmlEngine>
#include <QQuickWidget>
#include <QUrl>

ClaimPlugin::ClaimPlugin(QObject* parent) : QObject(parent) {}

ClaimPlugin::~ClaimPlugin() = default;

QWidget* ClaimPlugin::createWidget(LogosAPI* /*api*/) {
    // The bridge shells out to the local `airdrop` CLI to build a recipient's
    // claim arguments; submission to the chain is via `spel` on the same host.
    m_bridge = new ClaimBridge(this);

    // Qt's resource system is process-global: two modules that both register
    // /qml/Main.qml resolve to whichever registered first, so with a second
    // module installed one tile drew the other's UI over its own data.
    // The prefix is this module's name, which cannot collide.
    auto* view = new QQuickWidget();
    view->engine()->rootContext()->setContextProperty(
        QStringLiteral("bridge"), m_bridge);
    view->setResizeMode(QQuickWidget::SizeRootObjectToView);
    view->setSource(QUrl(QStringLiteral("qrc:/lp_0003_airdrop/Main.qml")));
    return view;
}

void ClaimPlugin::destroyWidget(QWidget* widget) {
    if (widget) {
        widget->deleteLater();
    }
}
