/* Fixed system values - board has 16MB flash available */
_flash_start = 0x10000000;
_flash_size = 16M;
_boot2_size = 0x100;
_end_of_flash_memory = 0x10000000 + _flash_size;
_ram_start = 0x20000000;
_ram_size = 264K;
_page_size = 4k;

/* Embassy bootloader locations and sizes */
_bootloader_start = _flash_start + _boot2_size;
_bootloader_size = 48K - _boot2_size; /* ensure page aligned */
_bootloader_end = _bootloader_start + _bootloader_size;

_bootloader_state_start = _bootloader_end;
_bootloader_state_size = 4K;
_bootloader_state_end = _bootloader_state_start + _bootloader_state_size;

_bootloader_active_partition_start = _bootloader_state_end;
_bootloader_active_partition_size = 2M;
_bootloader_active_partition_end = _bootloader_active_partition_start + _bootloader_active_partition_size;

_bootloader_update_partition_start = _bootloader_active_partition_end;
_bootloader_update_partition_size = 2M + _page_size;
_bootloader_update_partition_end = _bootloader_update_partition_start + _bootloader_update_partition_size;

/* Application storage values - located at the end of flash */
_historical_log_size = 32k;
_settings_storage_size = 8k;
_app_nvm_total_size = _historical_log_size + _settings_storage_size;
_app_nvm_start = _end_of_flash_memory - _app_nvm_total_size;