# Design Notes

This provides some high level information regarding the design of the software to give a
foothold into adding common features.

## Settings data flow

Settings requires and interaction between the UI, application manager and the target component where
the setting is used. Each has their own responsibility.

The UI has the responsibility to display the settings, manage the input of the setting of the user and constrain
it to the desired options the user is to have available and their mapping to something useful to the user
(e.g., High / Medium / Low LED brightness which can map to values in the range 0-255).

### Setting flow 1 (deprecated)

When the user makes an update the UI sends a UI action message to the application as a `UiActionsMessage`. The
application then forwards the setting change request to the components as a `ApplicationMessage::ApplicationDataUpdate`.

When a component receives the new setting data applicable to their function it saves the setting via the settings
accessor.

### Setting flow 2

The UI directly uses the SettingsAccessor to save the setting value as set by the user. This triggers a message
to be sent out on the settings channel so that any subscribed interested parties can handle the updated setting.

## Settings storage

Settings storage is interacted with via the `SettingsStorage` trait. In this application this
is implemented by the `FlashSettingsAccessor`.

The settings accessor should be passed in as a dependency to objects that require it or it can be instantiated directly
if you want.