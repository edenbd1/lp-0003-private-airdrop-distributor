// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Top-level Qt plugin object for the LP-0003 airdrop claim surface. Owns the
// QQuickWidget that hosts the QML scene and exposes the ClaimBridge to it as a
// context property.

#pragma once

#include <QObject>
#include <QString>
#include <QWidget>

// LogosAPI is forward-declared rather than included here so this header builds
// standalone in the IDE-only preview-app path.
class LogosAPI;
class ClaimBridge;

// Basecamp's IComponent interface, declared here so the manual build path does
// not need the SDK header on the include path.
class IComponent {
public:
    virtual ~IComponent() = default;
    virtual QString name() const = 0;
    virtual QWidget* createWidget(LogosAPI* api) = 0;
    virtual void destroyWidget(QWidget* widget) = 0;
};

Q_DECLARE_INTERFACE(IComponent, "com.networkschool.logos.IComponent/1.0")

#define ClaimPlugin_IID "com.networkschool.lp0003.ClaimPlugin/1.0"

class ClaimPlugin : public QObject, public IComponent {
    Q_OBJECT
    Q_PLUGIN_METADATA(IID ClaimPlugin_IID FILE "metadata.json")
    Q_INTERFACES(IComponent)

public:
    explicit ClaimPlugin(QObject* parent = nullptr);
    ~ClaimPlugin() override;

    QString  name() const override { return QStringLiteral("lp_0003_airdrop"); }
    QWidget* createWidget(LogosAPI* api) override;
    void     destroyWidget(QWidget* widget) override;

private:
    ClaimBridge* m_bridge = nullptr;
};
