extern crate std;

const WASM_PAGE_SZ: usize = 65536;

pub struct FastLZWasm {
    module: wasmi::Module,
    linker: wasmi::Linker<()>,
    store: wasmi::Store<()>,
}
impl FastLZWasm {
    pub fn new() -> Self {
        let engine = wasmi::Engine::default();

        let srcdir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let wasm_fn = srcdir.join("testtool/fastlz.wasm");
        let wasm_f = std::fs::File::open(wasm_fn).unwrap();
        let module = wasmi::Module::new_streaming(&engine, wasm_f).unwrap();

        let linker: wasmi::Linker<()> = wasmi::Linker::new(&engine);

        let store: wasmi::Store<()> = wasmi::Store::new(&engine, ());

        Self {
            module,
            linker,
            store,
        }
    }

    pub fn fastlz_compress_level<'s>(&'s mut self, level: u32, input_data: &[u8]) -> &'s [u8] {
        let instance = self
            .linker
            .instantiate(&mut self.store, &self.module)
            .unwrap()
            .start(&mut self.store)
            .unwrap();

        let fastlz_compress_level = instance
            .get_typed_func::<(u32, u32, u32, u32), u32>(&mut self.store, "fastlz_compress_level")
            .unwrap();
        let mem = instance.get_memory(&mut self.store, "memory").unwrap();

        // expand memory to store input and output
        // assume max expansion factor of 16 (way too much already)
        // [stack] [unused] | [input data] [output space] |
        //                  ^ initial memory size
        //                                                ^ expanded memory size

        let to_alloc_bytes = input_data.len() + input_data.len() * 16;
        let to_alloc_pages = (to_alloc_bytes + WASM_PAGE_SZ - 1) / WASM_PAGE_SZ;

        let cur_mem_sz_pages = mem.grow(&mut self.store, to_alloc_pages as u32).unwrap();
        let cur_mem_sz_bytes = cur_mem_sz_pages as usize * WASM_PAGE_SZ;

        // copy data in
        mem.data_mut(&mut self.store)[cur_mem_sz_bytes..cur_mem_sz_bytes + input_data.len()]
            .copy_from_slice(input_data);

        // run
        let out_len = fastlz_compress_level
            .call(
                &mut self.store,
                (
                    level,
                    cur_mem_sz_bytes as u32,
                    input_data.len() as u32,
                    (cur_mem_sz_bytes + input_data.len()) as u32,
                ),
            )
            .unwrap();

        // borrow data out
        let out_data = &mem.data(&mut self.store)[cur_mem_sz_bytes + input_data.len()
            ..cur_mem_sz_bytes + input_data.len() + out_len as usize];

        out_data
    }

    pub fn fastlz_decompress<'s>(&'s mut self, input_data: &[u8]) -> &'s [u8] {
        let instance = self
            .linker
            .instantiate(&mut self.store, &self.module)
            .unwrap()
            .start(&mut self.store)
            .unwrap();

        let fastlz_decompress = instance
            .get_typed_func::<(u32, u32, u32, u32), u32>(&mut self.store, "fastlz_decompress")
            .unwrap();
        let mem = instance.get_memory(&mut self.store, "memory").unwrap();

        // assume max expansion factor of 1024
        let to_alloc_bytes = input_data.len() + input_data.len() * 1024;
        let to_alloc_pages = (to_alloc_bytes + WASM_PAGE_SZ - 1) / WASM_PAGE_SZ;

        let cur_mem_sz_pages = mem.grow(&mut self.store, to_alloc_pages as u32).unwrap();
        let cur_mem_sz_bytes = cur_mem_sz_pages as usize * WASM_PAGE_SZ;

        // copy data in
        mem.data_mut(&mut self.store)[cur_mem_sz_bytes..cur_mem_sz_bytes + input_data.len()]
            .copy_from_slice(input_data);

        // run
        let out_len = fastlz_decompress
            .call(
                &mut self.store,
                (
                    cur_mem_sz_bytes as u32,
                    input_data.len() as u32,
                    (cur_mem_sz_bytes + input_data.len()) as u32,
                    (input_data.len() * 1024) as u32,
                ),
            )
            .unwrap();

        // borrow data out
        let out_data = &mem.data(&mut self.store)[cur_mem_sz_bytes + input_data.len()
            ..cur_mem_sz_bytes + input_data.len() + out_len as usize];

        out_data
    }
}
