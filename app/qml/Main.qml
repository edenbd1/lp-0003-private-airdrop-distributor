// SPDX-License-Identifier: MIT OR Apache-2.0
//
// LP-0003 airdrop claim pane. `bridge` is the ClaimBridge instance injected as a
// context property by plugin.cpp; it shells out to the local `airdrop` CLI to
// build a recipient's claim arguments, which are then submitted with `spel`.

import QtQuick 6
import QtQuick.Controls 6
import QtQuick.Layouts 6

Rectangle {
    id: root
    color: "#0d1117"
    anchors.fill: parent

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 24
        spacing: 14

        Label {
            text: "Private Airdrop Claim"
            color: "#f0f6fc"
            font.pixelSize: 24
            font.bold: true
        }
        Label {
            text: "Prove you are in a committed eligibility set and claim your allocation, without revealing which address you hold."
            color: "#8b949e"
            font.pixelSize: 13
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: 8
            Label { text: "Distribution dir"; color: "#c9d1d9"; Layout.preferredWidth: 120 }
            TextField {
                id: distDir
                Layout.fillWidth: true
                placeholderText: "path produced by `airdrop demo-distribution`"
                text: ""
            }
        }
        RowLayout {
            Layout.fillWidth: true
            spacing: 8
            Label { text: "Recipient index"; color: "#c9d1d9"; Layout.preferredWidth: 120 }
            SpinBox { id: idx; from: 0; to: 9999; value: 0 }
            Item { Layout.fillWidth: true }
            Button {
                text: "Build claim"
                onClicked: {
                    output.text = bridge.buildClaimArgs(distDir.text, idx.value,
                                                        distDir.text + "/claim_gui.args")
                }
            }
        }

        Rectangle {
            Layout.fillWidth: true
            Layout.fillHeight: true
            color: "#161b22"
            radius: 6
            border.color: "#30363d"
            ScrollView {
                anchors.fill: parent
                anchors.margins: 12
                TextArea {
                    id: output
                    readOnly: true
                    color: "#8ddb8d"
                    font.family: "monospace"
                    font.pixelSize: 12
                    wrapMode: TextArea.WrapAnywhere
                    text: "Point at a distribution directory, pick your recipient index, and build your claim. The arguments are written next to the distribution; submit them with:\n\n  spel --idl idl/claim_verifier.idl.json --program artifacts/programs/claim_verifier.bin \\\n    --bin-claimlez artifacts/programs/claim_lez.bin \\\n    -- claim --claimant Private/<your-account> $(cat claim_gui.args)"
                }
            }
        }
    }
}
