# Task Notes

This is a scratch pad of notes to track ideas. Don't take it too seriously.

## Software

### Bugs / Things to fix

* Device and system menu requires two clicks down from top before scrolling starts
* Re-entering settings menu goes back to 'exit' entry

### Feature List

* Time to drink reminder mode (flash LED when time since last drink has elapsed)
* Monitoring functionality
    * ~~Monitor drink in background if in settings menu~~
    * ~~Daily/hourly target is settable~~
    * ~~Colour of LEDs changes in monitoring mode based on whether consumption is high/low/on target.~~
    * ~~Multiple monitoring screen support~~
    * ~~Show on screen target status~~
    * ~~Save active monitoring screen, so it is recalled after power cycle~~
    * ~~Reset consumption at midnight~~
    * ~~Set time to achieve consumption by (for daily mode)~~
    * ~~Use time to achieve consumption by (for daily mode)~~
    * **NEXT** Add current consumption rate (last hour) as well as having daily hourly rate
    * Retain last consumption data between power cycles, respect reset point
    * Manual add/subtract consumption
    * Celebration screen when target achieved for the day (for daily mode)
    * Display ideas
        * Show visual representation of target 'flight path'
        * Show required drink amount to get back on target
        * Rate history as bar graph over last N hours
        * Progress bar to a daily target
    * Build custom screens based on standard 'widgets' (3x2 grid?)
    * Behaviours when vessel is missing
        * Time based 'aggressiveness' - no cup for long period = more vigorous LEDs
* Historical log of consumption
    * ~~Store history~~
    * can be viewed as a graph on device (sparkline?)
    * or transferred via USB.
* Firmware update via USB
* Live consumption data streamed over USB to be accessible by a local web page
* Timezones
    * Daylight savings
* Visual alarm
* Pomodoro mode
* ~~Screen off after period of inactivity to save OLED~~

### Application logic ideas

* ~~Large negative change in consumption indicates refill => ignore~~
* Large consumption value may indicate change in vessel or drank a lot. Options:
    * Store a long term history or heuristic to ask if new cup on outliers?
    * Have the above threshold settable in the settings menu
* Vessel off for a long time resets to 'waiting for activity' - so we assume new cup when activity is then detected.
    * Settable timeout?

### Settings menu

* LEDs
    * ~~LED brightness~~
    * Inactivity timeout
    * LED indicates missing vessel - On/Off
    * LED indicates outside target - On/Off
    * LED no motion mode
* Target / monitoring
    * ~~Daily / Hourly~~
    * ~~Target Value~~
    * ~~Day length (?) / day resets at HH:MM / achieve daily target by HH:MM~~
        * Day start? / end
    * Target is minimum or maximum (decides which way is over/under consumption)
    * (?) Reset current consumption -> puts directly back on the target 'flight path'
    * Large consumption threshold (to ask if new cup)
    * LED visualisation thresholds
        * colour selection?
    * (?) Reset vessel if vessel not present for X minutes
* Display
    * ~~Brightness~~
    * ~~Inactivity timeout~~
* Sys config
    * ~~Test mode access~~
    * ~~Calibration~~
    * ~~Set Date/Time~~
        * ~~Improve date/time setting screen to be on par with numerical setting~~
    * TZ setting
        * see https://docs.rs/time-tz/latest/time_tz/timezones/index.html / https://crates.io/crates/chrono-tz
    * Factory Reset
* ~~'About' screen~~
* Auto exit settings after period of time inactive
* ~~Continue to monitor drink in background~~

### LEDs

* Low priority - Smooth transition between states

### NVM

* ~~Store calibration~~
* ~~Store settings~~
* ~~Key / Value pair / general settings store~~
* ~~Read / write~~
* **WIP** Historical data store / timestamped sequential data
    * ~~Write~~
    * Read
* Error log
* Factory reset

### De-prioritised features

* Battery support - HW supported
    * Battery level indicator
    * Battery mode (reduced LEDs)

* SD Card - no HW support, onboard flash is 16MB currently.
    * Logging data
    * Logging detailed errors

* USB data transfer

* Speaker - no HW support
    * Tones / Clicks / Alarms

# Enclosure

* Light barrier for PSU LEDs
* Resolve enclosure interference w/ drink pad which is potentially affecting the monitoring.
