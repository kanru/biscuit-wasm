use biscuit_auth as biscuit;
use wasm_bindgen::prelude::*;

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

/// a Biscuit token
///
/// it can produce an attenuated or sealed token, or be used
/// in an authorizer along with Datalog policies
#[wasm_bindgen]
pub struct Biscuit(biscuit::Biscuit);

#[wasm_bindgen]
impl Biscuit {
    /// Creates a BiscuitBuilder
    ///
    /// the builder can then create a new token with a root key
    pub fn builder() -> BiscuitBuilder {
        BiscuitBuilder::new()
    }

    /// Creates a BlockBuilder to prepare for attenuation
    ///
    /// the bulder can then be given to the token's append method to create an attenuated token
    pub fn create_block(&self) -> BlockBuilder {
        BlockBuilder(self.0.create_block())
    }

    /// Creates an attenuated token by adding the block generated by the BlockBuilder
    pub fn append(&self, block: BlockBuilder) -> Result<Biscuit, JsValue> {
        Ok(Biscuit(
            self.0
                .append(block.0)
                .map_err(|e| JsValue::from_serde(&e).unwrap())?,
        ))
    }

    /// Creates an authorizer from the token
    pub fn authorizer(&self) -> Result<Authorizer, JsValue> {
        Ok(Authorizer {
            token: Some(self.0.clone()),
            ..Authorizer::default()
        })
    }

    /// Seals the token
    ///
    /// A sealed token cannot be attenuated
    pub fn seal(&self) -> Result<Biscuit, JsValue> {
        Ok(Biscuit(
            self.0
                .seal()
                .map_err(|e| JsValue::from_serde(&e).unwrap())?,
        ))
    }

    /// Deserializes a token from raw data
    ///
    /// This will check the signature using the root key
    pub fn from_bytes(data: &[u8], root: &PublicKey) -> Result<Biscuit, JsValue> {
        Ok(Biscuit(
            biscuit::Biscuit::from(data, |_| root.0)
                .map_err(|e| JsValue::from_serde(&e).unwrap())?,
        ))
    }

    /// Deserializes a token from URL safe base 64 data
    ///
    /// This will check the signature using the root key
    pub fn from_base64(data: &str, root: &PublicKey) -> Result<Biscuit, JsValue> {
        Ok(Biscuit(
            biscuit::Biscuit::from_base64(data, |_| root.0)
                .map_err(|e| JsValue::from_serde(&e).unwrap())?,
        ))
    }

    /// Serializes to raw data
    pub fn to_bytes(&self) -> Result<Box<[u8]>, JsValue> {
        Ok(self
            .0
            .to_vec()
            .map_err(|e| JsValue::from_serde(&e).unwrap())?
            .into_boxed_slice())
    }

    /// Serializes to URL safe base 64 data
    pub fn to_base64(&self) -> Result<String, JsValue> {
        Ok(self
            .0
            .to_base64()
            .map_err(|e| JsValue::from_serde(&e).unwrap())?)
    }

    /// Returns the list of revocation identifiers, encoded as URL safe base 64
    pub fn revocation_identifiers(&self) -> Box<[JsValue]> {
        let ids: Vec<_> = self
            .0
            .revocation_identifiers()
            .into_iter()
            .map(|id| base64::encode_config(id, base64::URL_SAFE).into())
            .collect();
        ids.into_boxed_slice()
    }

    /// Returns the number of blocks in the token
    pub fn block_count(&self) -> usize {
        self.0.block_count()
    }

    /// Prints a block's content as Datalog code
    pub fn block_source(&self, index: usize) -> Option<String> {
        self.0.print_block_source(index)
    }
}

/// The Authorizer verifies a request according to its policies and the provided token
#[wasm_bindgen]
#[derive(Default)]
pub struct Authorizer {
    token: Option<biscuit::Biscuit>,
    facts: Vec<biscuit::builder::Fact>,
    rules: Vec<biscuit::builder::Rule>,
    checks: Vec<biscuit::builder::Check>,
    policies: Vec<biscuit::builder::Policy>,
}

#[wasm_bindgen]
impl Authorizer {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Authorizer {
        Authorizer::default()
    }

    /// Adds a Datalog fact
    pub fn add_fact(&mut self, fact: &str) -> Result<(), JsValue> {
        self.facts.push(
            fact.try_into()
                .map_err(|e| JsValue::from_serde(&e).unwrap())?,
        );
        Ok(())
    }

    /// Adds a Datalog rule
    pub fn add_rule(&mut self, rule: &str) -> Result<(), JsValue> {
        self.rules.push(
            rule.try_into()
                .map_err(|e| JsValue::from_serde(&e).unwrap())?,
        );
        Ok(())
    }

    /// Adds a check
    ///
    /// All checks, from authorizer and token, must be validated to authorize the request
    pub fn add_check(&mut self, check: &str) -> Result<(), JsValue> {
        self.checks.push(
            check
                .try_into()
                .map_err(|e| JsValue::from_serde(&e).unwrap())?,
        );
        Ok(())
    }

    /// Adds a policy
    ///
    /// The authorizer will test all policies in order of addition and stop at the first one that
    /// matches. If it is a "deny" policy, the request fails, while with an "allow" policy, it will
    /// succeed
    pub fn add_policy(&mut self, policy: &str) -> Result<(), JsValue> {
        self.policies.push(
            policy
                .try_into()
                .map_err(|e| JsValue::from_serde(&e).unwrap())?,
        );
        Ok(())
    }

    /// Adds facts, rules, checks and policies as one code block
    pub fn add_code(&mut self, source: &str) -> Result<(), JsValue> {
        let source_result = biscuit::parser::parse_source(source).map_err(|e| {
            let e: biscuit::error::Token = e.into();
            JsValue::from_serde(&e).unwrap()
        })?;

        for (_, fact) in source_result.facts.into_iter() {
            self.facts.push(fact);
        }

        for (_, rule) in source_result.rules.into_iter() {
            self.rules.push(rule);
        }

        for (_, check) in source_result.checks.into_iter() {
            self.checks.push(check);
        }

        for (_, policy) in source_result.policies.into_iter() {
            self.policies.push(policy);
        }

        Ok(())
    }

    /// Runs the authorization checks and policies
    ///
    /// Returns the index of the matching allow policy, or an error containing the matching deny
    /// policy or a list of the failing checks
    pub fn authorize(&self) -> Result<usize, JsValue> {
        let mut authorizer = match &self.token {
            Some(token) => token
                .authorizer()
                .map_err(|e| JsValue::from_serde(&e).unwrap())?,
            None => biscuit::Authorizer::new().map_err(|e| JsValue::from_serde(&e).unwrap())?,
        };

        for fact in self.facts.iter() {
            authorizer
                .add_fact(fact.clone())
                .map_err(|e| JsValue::from_serde(&e).unwrap())?;
        }
        for rule in self.rules.iter() {
            authorizer
                .add_rule(rule.clone())
                .map_err(|e| JsValue::from_serde(&e).unwrap())?;
        }
        for check in self.checks.iter() {
            authorizer
                .add_check(check.clone())
                .map_err(|e| JsValue::from_serde(&e).unwrap())?;
        }
        for policy in self.policies.iter() {
            authorizer
                .add_policy(policy.clone())
                .map_err(|e| JsValue::from_serde(&e).unwrap())?;
        }

        Ok(authorizer
            .authorize()
            .map_err(|e| JsValue::from_serde(&e).unwrap())?)
    }
}

/// Creates a token
#[wasm_bindgen]
pub struct BiscuitBuilder {
    facts: Vec<biscuit::builder::Fact>,
    rules: Vec<biscuit::builder::Rule>,
    checks: Vec<biscuit::builder::Check>,
}

#[wasm_bindgen]
impl BiscuitBuilder {
    fn new() -> BiscuitBuilder {
        BiscuitBuilder {
            facts: Vec::new(),
            rules: Vec::new(),
            checks: Vec::new(),
        }
    }

    pub fn build(self, root: &KeyPair) -> Result<Biscuit, JsValue> {
        let mut builder = biscuit_auth::Biscuit::builder(&root.0);
        for fact in self.facts.into_iter() {
            builder
                .add_authority_fact(fact)
                .map_err(|e| JsValue::from_serde(&e).unwrap())?;
        }
        for rule in self.rules.into_iter() {
            builder
                .add_authority_rule(rule)
                .map_err(|e| JsValue::from_serde(&e).unwrap())?;
        }
        for check in self.checks.into_iter() {
            builder
                .add_authority_check(check)
                .map_err(|e| JsValue::from_serde(&e).unwrap())?;
        }

        Ok(Biscuit(
            builder
                .build()
                .map_err(|e| JsValue::from_serde(&e).unwrap())?,
        ))
    }

    /// Adds a Datalog fact
    pub fn add_authority_fact(&mut self, fact: &str) -> Result<(), JsValue> {
        self.facts.push(
            fact.try_into()
                .map_err(|e| JsValue::from_serde(&e).unwrap())?,
        );
        Ok(())
    }

    /// Adds a Datalog rule
    pub fn add_authority_rule(&mut self, rule: &str) -> Result<(), JsValue> {
        self.rules.push(
            rule.try_into()
                .map_err(|e| JsValue::from_serde(&e).unwrap())?,
        );
        Ok(())
    }

    /// Adds a check
    ///
    /// All checks, from authorizer and token, must be validated to authorize the request
    pub fn add_authority_check(&mut self, check: &str) -> Result<(), JsValue> {
        self.checks.push(
            check
                .try_into()
                .map_err(|e| JsValue::from_serde(&e).unwrap())?,
        );
        Ok(())
    }
}

/// Creates a block to attenuate a token
#[wasm_bindgen]
pub struct BlockBuilder(biscuit::builder::BlockBuilder);

#[wasm_bindgen]
impl BlockBuilder {
    /// Adds a Datalog fact
    pub fn add_fact(&mut self, fact: &str) -> Result<(), JsValue> {
        Ok(self
            .0
            .add_fact(fact)
            .map_err(|e| JsValue::from_serde(&e).unwrap())?)
    }

    /// Adds a Datalog rule
    pub fn add_rule(&mut self, rule: &str) -> Result<(), JsValue> {
        Ok(self
            .0
            .add_rule(rule)
            .map_err(|e| JsValue::from_serde(&e).unwrap())?)
    }

    /// Adds a check
    ///
    /// All checks, from authorizer and token, must be validated to authorize the request
    pub fn add_check(&mut self, check: &str) -> Result<(), JsValue> {
        Ok(self
            .0
            .add_check(check)
            .map_err(|e| JsValue::from_serde(&e).unwrap())?)
    }

    /// Adds facts, rules, checks and policies as one code block
    pub fn add_code(&mut self, source: &str) -> Result<(), JsValue> {
        self.0
            .add_code(source)
            .map_err(|e| JsValue::from_serde(&e).unwrap())
    }
}

/// A pair of public and private key
#[wasm_bindgen]
pub struct KeyPair(biscuit::KeyPair);

#[wasm_bindgen]
impl KeyPair {
    #[wasm_bindgen(constructor)]
    pub fn new() -> KeyPair {
        KeyPair(biscuit::KeyPair::new())
    }

    pub fn public(&self) -> PublicKey {
        PublicKey(self.0.public())
    }

    pub fn private(&self) -> PrivateKey {
        PrivateKey(self.0.private())
    }
}

/// Public key
#[wasm_bindgen]
pub struct PublicKey(biscuit::PublicKey);

#[wasm_bindgen]
impl PublicKey {
    /// Serializes a public key to raw bytes
    pub fn to_bytes(&self, out: &mut [u8]) -> Result<(), JsValue> {
        if out.len() != 32 {
            return Err(JsValue::from_serde(&biscuit::error::Token::Format(
                biscuit::error::Format::InvalidKeySize(out.len()),
            ))
            .unwrap());
        }

        out.copy_from_slice(&self.0.to_bytes());
        Ok(())
    }

    /// Serializes a public key to a hexadecimal string
    pub fn to_hex(&self) -> String {
        hex::encode(&self.0.to_bytes())
    }

    /// Deserializes a public key from raw bytes
    pub fn from_bytes(&self, data: &[u8]) -> Result<PublicKey, JsValue> {
        let key = biscuit_auth::PublicKey::from_bytes(data)
            .map_err(|e| JsValue::from_serde(&e).unwrap())?;
        Ok(PublicKey(key))
    }

    /// Deserializes a public key from a hexadecimal string
    pub fn from_hex(&self, data: &str) -> Result<PublicKey, JsValue> {
        let data = hex::decode(data).map_err(|e| {
            JsValue::from_serde(&biscuit::error::Token::Format(
                    biscuit::error::Format::InvalidKey(format!("could not deserialize hex encoded key: {}", e)),
                )).unwrap()
        })?;
        let key = biscuit_auth::PublicKey::from_bytes(&data)
            .map_err(|e| JsValue::from_serde(&e).unwrap())?;
        Ok(PublicKey(key))
    }
}
#[wasm_bindgen]
pub struct PrivateKey(biscuit::PrivateKey);

#[wasm_bindgen]
impl PrivateKey {
    /// Serializes a private key to raw bytes
    pub fn to_bytes(&self, out: &mut [u8]) -> Result<(), JsValue> {
        if out.len() != 32 {
            return Err(JsValue::from_serde(&biscuit::error::Token::Format(
                biscuit::error::Format::InvalidKeySize(out.len()),
            ))
            .unwrap());
        }

        out.copy_from_slice(&self.0.to_bytes());
        Ok(())
    }

    /// Serializes a private key to a hexadecimal string
    pub fn to_hex(&self) -> String {
        hex::encode(&self.0.to_bytes())
    }

    /// Deserializes a private key from raw bytes
    pub fn from_bytes(&self, data: &[u8]) -> Result<PrivateKey, JsValue> {
        let key = biscuit_auth::PrivateKey::from_bytes(data)
            .map_err(|e| JsValue::from_serde(&e).unwrap())?;
        Ok(PrivateKey(key))
    }

    /// Deserializes a private key from a hexadecimal string
    pub fn from_hex(&self, data: &str) -> Result<PrivateKey, JsValue> {
        let data = hex::decode(data).map_err(|e| {
            JsValue::from_serde(&biscuit::error::Token::Format(
                    biscuit::error::Format::InvalidKey(format!("could not deserialize hex encoded key: {}", e)),
                )).unwrap()
        })?;
        let key = biscuit_auth::PrivateKey::from_bytes(&data)
            .map_err(|e| JsValue::from_serde(&e).unwrap())?;
        Ok(PrivateKey(key))
    }
}

#[wasm_bindgen]
extern "C" {
    // Use `js_namespace` here to bind `console.log(..)` instead of just
    // `log(..)`
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

#[wasm_bindgen(start)]
pub fn init() {
    wasm_logger::init(wasm_logger::Config::default());
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));

    log("biscuit-wasm loading")
}
