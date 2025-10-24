/** *** CRITICAL WARNING ***
 * This file uses values from memory_common.x to ensure alignment between
 * the bootloader and the application.
 *
 * Consistency between the bootloader and the application is important
 * for the correct operation of the application, bootloader and update mechanism.
 */

INCLUDE "memory_common.x"

MEMORY
{
    BOOT2               : ORIGIN = _flash_start,                        LENGTH = _boot2_size
    BOOTLOADER_STATE    : ORIGIN = _bootloader_state_start,             LENGTH = _bootloader_state_size
    FLASH               : ORIGIN = _bootloader_active_partition_start,  LENGTH = _bootloader_active_partition_size
    DFU                 : ORIGIN = _bootloader_update_partition_start,  LENGTH = _bootloader_update_partition_size
    NVM                 : ORIGIN = _app_nvm_start,                      LENGTH = _app_nvm_total_size
    RAM                 : ORIGIN = _ram_start,                          LENGTH = _ram_size
}

/* Values from reference implementation - https://github.com/embassy-rs/embassy/blob/main/examples/boot/application/rp/memory.x */
__bootloader_state_start = ORIGIN(BOOTLOADER_STATE) - ORIGIN(BOOT2);
__bootloader_state_end = ORIGIN(BOOTLOADER_STATE) + LENGTH(BOOTLOADER_STATE) - ORIGIN(BOOT2);

__bootloader_dfu_start = ORIGIN(DFU) - ORIGIN(BOOT2);
__bootloader_dfu_end = ORIGIN(DFU) + LENGTH(DFU) - ORIGIN(BOOT2);
