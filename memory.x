/* actual board has 16MB flash available */
_flash_size = 2048k;
_historical_log_size = 32k;
_settings_storage_size = 8k;

_end_of_flash_memory = 0x10000000 + _flash_size;
_nvm_total_size = _historical_log_size + _settings_storage_size;
_nvm_start = _end_of_flash_memory - _nvm_total_size;

_boot2_size = 0x100;

MEMORY
{
    BOOT2   : ORIGIN = 0x10000000, LENGTH = _boot2_size
    FLASH   : ORIGIN = 0x10000100, LENGTH = _flash_size - (_nvm_total_size+_boot2_size)
    NVM     : ORIGIN = _nvm_start, LENGTH = _nvm_total_size
    RAM     : ORIGIN = 0x20000000, LENGTH = 264K
}
