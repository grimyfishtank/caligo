declare module "circomlibjs" {
  export function buildPoseidon(): Promise<any>;
}

declare module "snarkjs" {
  export namespace groth16 {
    function fullProve(
      input: Record<string, string | string[]>,
      wasmFile: string,
      zkeyFile: string
    ): Promise<{ proof: any; publicSignals: string[] }>;

    function verify(
      vk: any,
      publicSignals: string[],
      proof: any
    ): Promise<boolean>;
  }
}
