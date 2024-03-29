# LibVMI

libmicrovmi can replace the low-level layer of LibVMI drivers:

![libvmi_libmicrovmi](https://user-images.githubusercontent.com/964610/127520458-d9fb6048-7682-4dbd-beb6-1c999395b1ff.png)

- [project](https://github.com/libvmi/libvmi)
- [issue](https://github.com/Wenzel/libmicrovmi/issues/137)
- [fork](https://github.com/Wenzel/libvmi/tree/libmicrovmi) (Note: use the `libmicrovmi` branch)
- compatibility: 🟠

### API Compatibility Status

LibVMI driver layer could be replaced by libmicrovmi.

The API is used in the following files:

- [driver_interface.c](https://github.com/libvmi/libvmi/blob/1ae39506b088d7b03cc2c6d6e0413be37f7ee8f5/libvmi/driver/driver_interface.h)
- [driver_wrapper.h](https://github.com/libvmi/libvmi/blob/1ae39506b088d7b03cc2c6d6e0413be37f7ee8f5/libvmi/driver/driver_wrapper.h)
- [memory_cache.h](https://github.com/Wenzel/libvmi/blob/libmicrovmi/libvmi/driver/memory_cache.h)

| API                                       | Supported | Observations |
|-------------------------------------------|:-----------:|--------------|
| `driver_init_mode()`                      |    🟠      |              |
| `driver_init()`                           |    🟠      |              |
| `driver_init_vmi()`                       |    🟠      |              |
| `driver_domainwatch_init()`               |            |              |
| `driver_destroy()`                        |    ✅      |              |
| `driver_get_id_from_name()`               |            |              |
| `driver_get_name_from_id()`               |            |              |
| `driver_get_id_from_uuid()`               |            |              |
| `driver_get_id()`                         |            |              |
| `driver_set_id()`                         |            |              |
| `driver_check_id()`                       |            |              |
| `driver_get_name()`                       |            |              |
| `driver_set_name()`                       |            |              |
| `driver_get_xsave_info()`                 |            |              |
| `driver_get_memsize()`                    |            |              |
| `driver_request_page_fault()`             |            |              |
| `driver_get_tsc_info()`                   |            |              |
| `driver_get_vcpumtrr()`                   |            |              |
| `driver_get_vcpureg()`                    |     ✅     |              |
| `driver_get_vcpuregs()`                   |            |              |
| `driver_set_vcpureg()`                    |            |              |
| `driver_set_vcpuregs()`                   |            |              |
| `driver_mmap_guest()`                     |            |              |
| `driver_write()`                          |            |              |
| `driver_is_pv()`                          |            |              |
| `driver_pause_vm()`                       |     ✅     |              |
| `driver_resume_vm()`                      |     ✅     |              |
| `driver_events_listen()`                  |            |              |
| `driver_are_events_pending()`             |            |              |
| `driver_set_reg_access()`                 |            |              |
| `driver_set_intr_access()`                |            |              |
| `driver_set_mem_access()`                 |            |              |
| `driver_start_single_step()`              |            |              |
| `driver_stop_single_step()`               |            |              |
| `driver_shutdown_single_step()`           |            |              |
| `driver_set_guest_requested()`            |            |              |
| `driver_set_cpuid_event()`                |            |              |
| `driver_set_debug_event()`                |            |              |
| `driver_set_privcall_event()`             |            |              |
| `driver_set_desc_access_event()`          |            |              |
| `driver_set_failed_emulation_event()`     |            |              |
| `driver_set_domain_watch_event()`         |            |              |
| `driver_slat_get_domain_state()`          |            |              |
| `driver_slat_set_domain_state()`          |            |              |
| `driver_slat_create()`                    |            |              |
| `driver_slat_destroy()`                   |            |              |
| `driver_slat_switch()`                    |            |              |
| `driver_slat_change_gfn()`                |            |              |
| `driver_set_access_required()`            |            |              |
| `get_data()`                              |     ✅     |              |
| `release_data()`                          |     ✅     |              |
