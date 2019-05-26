// The two ends.
extern const char _binary_symbols_txt_start[];
extern const char _binary_symbols_txt_end[];
void lkm_api_add_kernel_symbols(void* start, void* end);
void init_module(){
    lkm_api_add_kernel_symbols(&_binary_symbols_txt_start, &_binary_symbols_txt_end);
}
