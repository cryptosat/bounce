extern crate num_bigint;
extern crate pkcs11;

use num_bigint::BigUint;
use pkcs11::{errors::Error, types::*, Ctx};
use std::io;
use std::mem;
use std::path::PathBuf;
use std::{env, ptr};

fn pkcs11_module_name() -> PathBuf {
    let default_path =
        option_env!("PKCS11_SOFTHSM2_MODULE").unwrap_or("/usr/local/lib/softhsm/libsofthsm2.so");
    let path = env::var_os("PKCS11_SOFTHSM2_MODULE").unwrap_or_else(|| default_path.into());
    let path_buf = PathBuf::from(path);

    if !path_buf.exists() {
        panic!(
      "Could not find SoftHSM2 at `{}`. Set the `PKCS11_SOFTHSM2_MODULE` environment variable to \
       its location.",
      path_buf.display());
    }

    path_buf
}

/// This will create and initialize a context, set a SO and USER PIN, and login as the USER.
/// This is the starting point for all tests that are acting on the token.
/// If you look at the tests here in a "serial" manner, if all the tests are working up until
/// here, this will always succeed.
fn fixture_token() -> Result<(Ctx, CK_SESSION_HANDLE), Error> {
    let ctx = Ctx::new_and_initialize(pkcs11_module_name()).unwrap();
    let slots = ctx.get_slot_list(false).unwrap();
    let pin = Some("1234");
    const LABEL: &str = "rust-unit-test";
    let slot = *slots.first().ok_or(Error::Module("no slot available"))?;
    ctx.init_token(slot, pin, LABEL)?;
    let sh = ctx.open_session(slot, CKF_SERIAL_SESSION | CKF_RW_SESSION, None, None)?;
    ctx.login(sh, CKU_SO, pin)?;
    ctx.init_pin(sh, pin)?;
    ctx.logout(sh)?;
    ctx.login(sh, CKU_USER, pin)?;
    Ok((ctx, sh))
}

fn fixture_key_pair(
    ctx: &Ctx,
    sh: CK_SESSION_HANDLE,
    pubLabel: String,
    privLabel: String,
    signVerify: bool,
    encryptDecrypt: bool,
    recover: bool,
) -> Result<(CK_OBJECT_HANDLE, CK_OBJECT_HANDLE), Error> {
    let mechanism = CK_MECHANISM {
        mechanism: CKM_RSA_PKCS_KEY_PAIR_GEN,
        pParameter: ptr::null_mut(),
        ulParameterLen: 0,
    };

    let privClass = CKO_PRIVATE_KEY;
    let privKeyType = CKK_RSA;
    let privLabel = privLabel;
    let privToken = CK_TRUE;
    let privPrivate = CK_TRUE;
    let privSensitive = CK_TRUE;
    let privUnwrap = CK_FALSE;
    let privExtractable = CK_FALSE;
    let privSign = if signVerify { CK_TRUE } else { CK_FALSE };
    let privSignRecover = if recover { CK_TRUE } else { CK_FALSE };
    let privDecrypt = if encryptDecrypt { CK_TRUE } else { CK_FALSE };

    let privTemplate = vec![
        CK_ATTRIBUTE::new(CKA_CLASS).with_ck_ulong(&privClass),
        CK_ATTRIBUTE::new(CKA_KEY_TYPE).with_ck_ulong(&privKeyType),
        CK_ATTRIBUTE::new(CKA_LABEL).with_string(&privLabel),
        CK_ATTRIBUTE::new(CKA_TOKEN).with_bool(&privToken),
        CK_ATTRIBUTE::new(CKA_PRIVATE).with_bool(&privPrivate),
        CK_ATTRIBUTE::new(CKA_SENSITIVE).with_bool(&privSensitive),
        CK_ATTRIBUTE::new(CKA_UNWRAP).with_bool(&privUnwrap),
        CK_ATTRIBUTE::new(CKA_EXTRACTABLE).with_bool(&privExtractable),
        CK_ATTRIBUTE::new(CKA_SIGN).with_bool(&privSign),
        CK_ATTRIBUTE::new(CKA_SIGN_RECOVER).with_bool(&privSignRecover),
        CK_ATTRIBUTE::new(CKA_DECRYPT).with_bool(&privDecrypt),
    ];

    let pubClass = CKO_PUBLIC_KEY;
    let pubKeyType = CKK_RSA;
    let pubLabel = pubLabel;
    let pubToken = CK_TRUE;
    let pubPrivate = CK_TRUE;
    let pubWrap = CK_FALSE;
    let pubVerify = if signVerify { CK_TRUE } else { CK_FALSE };
    let pubVerifyRecover = if recover { CK_TRUE } else { CK_FALSE };
    let pubEncrypt = if encryptDecrypt { CK_TRUE } else { CK_FALSE };
    let pubModulusBits: CK_ULONG = 4096;
    let pubPublicExponent = BigUint::from(65537u32);
    let pubPublicExponentSlice = pubPublicExponent.to_bytes_le();

    let pubTemplate = vec![
        CK_ATTRIBUTE::new(CKA_CLASS).with_ck_ulong(&pubClass),
        CK_ATTRIBUTE::new(CKA_KEY_TYPE).with_ck_ulong(&pubKeyType),
        CK_ATTRIBUTE::new(CKA_LABEL).with_string(&pubLabel),
        CK_ATTRIBUTE::new(CKA_TOKEN).with_bool(&pubToken),
        CK_ATTRIBUTE::new(CKA_PRIVATE).with_bool(&pubPrivate),
        CK_ATTRIBUTE::new(CKA_WRAP).with_bool(&pubWrap),
        CK_ATTRIBUTE::new(CKA_VERIFY).with_bool(&pubVerify),
        CK_ATTRIBUTE::new(CKA_VERIFY_RECOVER).with_bool(&pubVerifyRecover),
        CK_ATTRIBUTE::new(CKA_ENCRYPT).with_bool(&pubEncrypt),
        CK_ATTRIBUTE::new(CKA_MODULUS_BITS).with_ck_ulong(&pubModulusBits),
        CK_ATTRIBUTE::new(CKA_PUBLIC_EXPONENT).with_biginteger(&pubPublicExponentSlice),
    ];

    let (pubOh, privOh) = ctx.generate_key_pair(sh, &mechanism, &pubTemplate, &privTemplate)?;
    Ok((pubOh, privOh))
}

fn fixture_token_and_key_pair(
) -> Result<(Ctx, CK_SESSION_HANDLE, CK_OBJECT_HANDLE, CK_OBJECT_HANDLE), Error> {
    let (ctx, sh) = fixture_token()?;
    let (pubOh, privOh) = fixture_key_pair(
        &ctx,
        sh,
        "rsa-pub".into(),
        "rsa-priv".into(),
        true,
        true,
        true,
    )?;
    Ok((ctx, sh, pubOh, privOh))
}

fn main() {
    println!("Enter your name: ");
    let mut name = String::new();
    io::stdin()
        .read_line(&mut name)
        .expect("Failed to read line");

    println!("Hello, {}!", &name[..name.len() - 1]);

    // Sign
    let (ctx, sh, _, privOh) = fixture_token_and_key_pair().unwrap();

    let parameter = CK_RSA_PKCS_PSS_PARAMS {
        hashAlg: CKM_SHA256,
        mgf: CKG_MGF1_SHA256,
        sLen: 32,
    };
    let mechanism = CK_MECHANISM {
        mechanism: CKM_SHA256_RSA_PKCS_PSS,
        pParameter: &parameter as *const _ as CK_VOID_PTR,
        ulParameterLen: mem::size_of::<CK_RSA_PKCS_PSS_PARAMS>() as CK_ULONG,
    };

    let res = ctx.sign_init(sh, &mechanism, privOh);
    assert!(
        res.is_ok(),
        "failed to call C_SignInit({}, {:?}, {}) with parameter: {}",
        sh,
        &mechanism,
        privOh,
        res.unwrap_err()
    );

    let data = name.into_bytes();
    let signature = ctx.sign(sh, &data);
    assert!(
        signature.is_ok(),
        "failed to call C_Sign({}, {:?}): {}",
        sh,
        &data,
        signature.unwrap_err()
    );
    let signature = signature.unwrap();
    println!("Signature bytes after C_Sign: {:?}", &signature);
}
