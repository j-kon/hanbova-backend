#![allow(clippy::not_unsafe_ptr_arg_deref)]

use cdk::{
    amount::{Amount, SplitTarget},
    nuts::{
        nut10::Conditions, nut11::SigFlag, CurrencyUnit, PaymentMethod, PublicKey, SecretKey,
        SpendingConditions,
    },
    wallet::{ReceiveOptions, SendOptions, Wallet},
};
use cdk_redb::WalletRedbDatabase;
use std::{
    cell::RefCell,
    ffi::{CStr, CString},
    os::raw::{c_char, c_int},
    path::Path,
    str::FromStr,
    sync::Arc,
};
use tokio::runtime::Runtime;

thread_local! {
    static LAST_ERROR: RefCell<String> = const { RefCell::new(String::new()) };
}

fn set_last_error(err: &str) {
    LAST_ERROR.with(|e| {
        *e.borrow_mut() = err.to_string();
    });
}

pub struct CdkWalletHandle {
    wallet: Arc<Wallet>,
    rt: Runtime,
}

#[no_mangle]
pub extern "C" fn hanbova_cdk_get_last_error() -> *mut c_char {
    LAST_ERROR.with(|e| {
        let err = e.borrow();
        if err.is_empty() {
            std::ptr::null_mut()
        } else {
            CString::new(err.as_str()).unwrap().into_raw()
        }
    })
}

#[no_mangle]
pub extern "C" fn hanbova_cdk_free_string(s: *mut c_char) {
    if !s.is_null() {
        unsafe {
            drop(CString::from_raw(s));
        }
    }
}

#[no_mangle]
pub extern "C" fn hanbova_cdk_wallet_create(
    mint_url: *const c_char,
    db_path: *const c_char,
    seed_hex: *const c_char,
    out_handle: *mut *mut CdkWalletHandle,
) -> c_int {
    if mint_url.is_null() || db_path.is_null() || seed_hex.is_null() || out_handle.is_null() {
        set_last_error("Null pointer provided to hanbova_cdk_wallet_create");
        return 1;
    }

    let mint_url_str = match unsafe { CStr::from_ptr(mint_url) }.to_str() {
        Ok(s) => s,
        Err(e) => {
            set_last_error(&format!("Invalid mint_url UTF-8: {e}"));
            return 2;
        }
    };

    let db_path_str = match unsafe { CStr::from_ptr(db_path) }.to_str() {
        Ok(s) => s,
        Err(e) => {
            set_last_error(&format!("Invalid db_path UTF-8: {e}"));
            return 3;
        }
    };

    let seed_hex_str = match unsafe { CStr::from_ptr(seed_hex) }.to_str() {
        Ok(s) => s,
        Err(e) => {
            set_last_error(&format!("Invalid seed_hex UTF-8: {e}"));
            return 4;
        }
    };

    let seed_bytes = match hex::decode(seed_hex_str) {
        Ok(b) if b.len() == 64 => {
            let mut arr = [0u8; 64];
            arr.copy_from_slice(&b);
            arr
        }
        Ok(b) => {
            set_last_error(&format!(
                "Seed must be exactly 64 bytes (128 hex chars), got {} bytes",
                b.len()
            ));
            return 5;
        }
        Err(e) => {
            set_last_error(&format!("Invalid hex seed: {e}"));
            return 6;
        }
    };

    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(r) => r,
        Err(e) => {
            set_last_error(&format!("Failed to create Tokio runtime: {e}"));
            return 7;
        }
    };

    let db_path_buf = if Path::new(db_path_str).is_dir() {
        Path::new(db_path_str).join("wallet.redb")
    } else {
        Path::new(db_path_str).to_path_buf()
    };
    if let Some(parent) = db_path_buf.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let db = match WalletRedbDatabase::new(&db_path_buf) {
        Ok(d) => Arc::new(d),
        Err(e) => {
            set_last_error(&format!("Failed to open Redb wallet database: {e}"));
            return 8;
        }
    };

    let wallet = match Wallet::new(mint_url_str, CurrencyUnit::Sat, db, seed_bytes, None) {
        Ok(w) => Arc::new(w),
        Err(e) => {
            set_last_error(&format!("Failed to instantiate CDK Wallet: {e}"));
            return 9;
        }
    };

    let handle = Box::new(CdkWalletHandle { wallet, rt });
    unsafe {
        *out_handle = Box::into_raw(handle);
    }

    0
}

#[no_mangle]
pub extern "C" fn hanbova_cdk_wallet_get_balance(
    handle: *mut CdkWalletHandle,
    out_spendable: *mut u64,
    out_pending: *mut u64,
) -> c_int {
    if handle.is_null() || out_spendable.is_null() || out_pending.is_null() {
        set_last_error("Null pointer provided to hanbova_cdk_wallet_get_balance");
        return 1;
    }

    let h = unsafe { &*handle };
    let res: Result<(u64, u64), String> = h.rt.block_on(async {
        let spendable = h.wallet.total_balance().await.map_err(|e| e.to_string())?;
        let pending = h
            .wallet
            .total_pending_balance()
            .await
            .map_err(|e| e.to_string())?;
        Ok((u64::from(spendable), u64::from(pending)))
    });

    match res {
        Ok((spendable, pending)) => {
            unsafe {
                *out_spendable = spendable;
                *out_pending = pending;
            }
            0
        }
        Err(e) => {
            set_last_error(&format!("Failed to get balance: {e}"));
            2
        }
    }
}

#[no_mangle]
pub extern "C" fn hanbova_cdk_mint_quote(
    handle: *mut CdkWalletHandle,
    amount_sats: u64,
    out_quote_id: *mut *mut c_char,
    out_invoice: *mut *mut c_char,
) -> c_int {
    if handle.is_null() || out_quote_id.is_null() || out_invoice.is_null() {
        set_last_error("Null pointer provided to hanbova_cdk_mint_quote");
        return 1;
    }

    let h = unsafe { &*handle };
    let res: Result<(String, String), String> = h.rt.block_on(async {
        let method = PaymentMethod::from_str("bolt11").map_err(|e| e.to_string())?;
        let quote = h
            .wallet
            .mint_quote(method, Some(Amount::from(amount_sats)), None, None)
            .await
            .map_err(|e| e.to_string())?;
        Ok((quote.id, quote.request))
    });

    match res {
        Ok((qid, inv)) => {
            unsafe {
                *out_quote_id = CString::new(qid).unwrap().into_raw();
                *out_invoice = CString::new(inv).unwrap().into_raw();
            }
            0
        }
        Err(e) => {
            set_last_error(&format!("Failed to create mint quote: {e}"));
            2
        }
    }
}

#[no_mangle]
pub extern "C" fn hanbova_cdk_mint(
    handle: *mut CdkWalletHandle,
    quote_id: *const c_char,
    out_minted_sats: *mut u64,
) -> c_int {
    if handle.is_null() || quote_id.is_null() || out_minted_sats.is_null() {
        set_last_error("Null pointer provided to hanbova_cdk_mint");
        return 1;
    }

    let quote_id_str = match unsafe { CStr::from_ptr(quote_id) }.to_str() {
        Ok(s) => s,
        Err(e) => {
            set_last_error(&format!("Invalid quote_id UTF-8: {e}"));
            return 2;
        }
    };

    let h = unsafe { &*handle };
    let res: Result<u64, String> = h.rt.block_on(async {
        let proofs = h
            .wallet
            .mint(quote_id_str, SplitTarget::default(), None)
            .await
            .map_err(|e| e.to_string())?;
        let total: u64 = proofs.iter().map(|p| u64::from(p.amount)).sum();
        Ok(total)
    });

    match res {
        Ok(minted) => {
            unsafe {
                *out_minted_sats = minted;
            }
            0
        }
        Err(e) => {
            set_last_error(&format!("Failed to mint proofs: {e}"));
            3
        }
    }
}

#[no_mangle]
pub extern "C" fn hanbova_cdk_melt_quote(
    handle: *mut CdkWalletHandle,
    invoice: *const c_char,
    out_quote_id: *mut *mut c_char,
    out_amount_sats: *mut u64,
    out_fee_reserve_sats: *mut u64,
) -> c_int {
    if handle.is_null()
        || invoice.is_null()
        || out_quote_id.is_null()
        || out_amount_sats.is_null()
        || out_fee_reserve_sats.is_null()
    {
        set_last_error("Null pointer provided to hanbova_cdk_melt_quote");
        return 1;
    }

    let inv_str = match unsafe { CStr::from_ptr(invoice) }.to_str() {
        Ok(s) => s,
        Err(e) => {
            set_last_error(&format!("Invalid invoice UTF-8: {e}"));
            return 2;
        }
    };

    let h = unsafe { &*handle };
    let res: Result<(String, u64, u64), String> = h.rt.block_on(async {
        let method = PaymentMethod::from_str("bolt11").map_err(|e| e.to_string())?;
        let quote = h
            .wallet
            .melt_quote(method, inv_str, None, None)
            .await
            .map_err(|e| e.to_string())?;
        Ok((
            quote.id,
            u64::from(quote.amount),
            u64::from(quote.fee_reserve),
        ))
    });

    match res {
        Ok((qid, amt, fee)) => {
            unsafe {
                *out_quote_id = CString::new(qid).unwrap().into_raw();
                *out_amount_sats = amt;
                *out_fee_reserve_sats = fee;
            }
            0
        }
        Err(e) => {
            set_last_error(&format!("Failed to create melt quote: {e}"));
            3
        }
    }
}

#[no_mangle]
pub extern "C" fn hanbova_cdk_melt(
    handle: *mut CdkWalletHandle,
    quote_id: *const c_char,
    out_paid: *mut c_int,
    out_preimage: *mut *mut c_char,
) -> c_int {
    if handle.is_null() || quote_id.is_null() || out_paid.is_null() || out_preimage.is_null() {
        set_last_error("Null pointer provided to hanbova_cdk_melt");
        return 1;
    }

    let qid_str = match unsafe { CStr::from_ptr(quote_id) }.to_str() {
        Ok(s) => s,
        Err(e) => {
            set_last_error(&format!("Invalid quote_id UTF-8: {e}"));
            return 2;
        }
    };

    let h = unsafe { &*handle };
    let res: Result<(bool, Option<String>), String> = h.rt.block_on(async {
        let prepared = h
            .wallet
            .prepare_melt(qid_str, std::collections::HashMap::new())
            .await
            .map_err(|e| e.to_string())?;
        let finalized = prepared.confirm().await.map_err(|e| e.to_string())?;
        let is_paid = finalized.state() == cdk::nuts::MeltQuoteState::Paid;
        let preimage = finalized.payment_proof().map(|s| s.to_string());
        Ok((is_paid, preimage))
    });

    match res {
        Ok((paid, preimage_opt)) => {
            unsafe {
                *out_paid = if paid { 1 } else { 0 };
                if let Some(preimage) = preimage_opt {
                    *out_preimage = CString::new(preimage).unwrap().into_raw();
                } else {
                    *out_preimage = std::ptr::null_mut();
                }
            }
            0
        }
        Err(e) => {
            set_last_error(&format!("Failed to melt quote: {e}"));
            3
        }
    }
}

#[no_mangle]
pub extern "C" fn hanbova_cdk_prepare_p2pk_send(
    handle: *mut CdkWalletHandle,
    amount_sats: u64,
    recipient_pubkey_hex: *const c_char,
    refund_pubkey_hex: *const c_char,
    locktime_unix: u64,
    out_token: *mut *mut c_char,
) -> c_int {
    if handle.is_null() || recipient_pubkey_hex.is_null() || out_token.is_null() {
        set_last_error("Null pointer provided to hanbova_cdk_prepare_p2pk_send");
        return 1;
    }

    let rec_pub_str = match unsafe { CStr::from_ptr(recipient_pubkey_hex) }.to_str() {
        Ok(s) => s,
        Err(e) => {
            set_last_error(&format!("Invalid recipient_pubkey_hex UTF-8: {e}"));
            return 2;
        }
    };

    let recipient_pubkey = match PublicKey::from_str(rec_pub_str) {
        Ok(p) => p,
        Err(e) => {
            set_last_error(&format!("Invalid secp256k1 recipient public key: {e}"));
            return 3;
        }
    };

    let refund_pubkeys = if !refund_pubkey_hex.is_null() {
        let ref_pub_str = match unsafe { CStr::from_ptr(refund_pubkey_hex) }.to_str() {
            Ok(s) if !s.is_empty() => s,
            _ => "",
        };
        if !ref_pub_str.is_empty() {
            match PublicKey::from_str(ref_pub_str) {
                Ok(p) => Some(vec![p]),
                Err(e) => {
                    set_last_error(&format!("Invalid refund public key: {e}"));
                    return 4;
                }
            }
        } else {
            None
        }
    } else {
        None
    };

    let conditions = Conditions::new(
        if locktime_unix > 0 {
            Some(locktime_unix)
        } else {
            None
        },
        None,
        refund_pubkeys,
        None,
        Some(SigFlag::SigInputs),
        None,
    )
    .map_err(|e| format!("Failed to create NUT-10 conditions: {e}"));

    let conditions = match conditions {
        Ok(c) => c,
        Err(e) => {
            set_last_error(&e);
            return 5;
        }
    };

    let spending_conditions = SpendingConditions::new_p2pk(recipient_pubkey, Some(conditions));

    let h = unsafe { &*handle };
    let res: Result<String, String> = h.rt.block_on(async {
        let send_opts = SendOptions {
            conditions: Some(spending_conditions),
            ..Default::default()
        };
        let prepared = h
            .wallet
            .prepare_send(Amount::from(amount_sats), send_opts)
            .await
            .map_err(|e| e.to_string())?;
        let token = prepared.confirm(None).await.map_err(|e| e.to_string())?;
        Ok(token.to_string())
    });

    match res {
        Ok(token_str) => {
            unsafe {
                *out_token = CString::new(token_str).unwrap().into_raw();
            }
            0
        }
        Err(e) => {
            set_last_error(&format!("Failed to prepare P2PK send: {e}"));
            6
        }
    }
}

#[no_mangle]
pub extern "C" fn hanbova_cdk_receive_p2pk(
    handle: *mut CdkWalletHandle,
    token_str: *const c_char,
    p2pk_privkey_hex: *const c_char,
    out_received_sats: *mut u64,
) -> c_int {
    if handle.is_null() || token_str.is_null() || out_received_sats.is_null() {
        set_last_error("Null pointer provided to hanbova_cdk_receive_p2pk");
        return 1;
    }

    let token = match unsafe { CStr::from_ptr(token_str) }.to_str() {
        Ok(s) => s,
        Err(e) => {
            set_last_error(&format!("Invalid token_str UTF-8: {e}"));
            return 2;
        }
    };

    let mut signing_keys = Vec::new();
    if !p2pk_privkey_hex.is_null() {
        let key_str = match unsafe { CStr::from_ptr(p2pk_privkey_hex) }.to_str() {
            Ok(s) => s.trim(),
            Err(_) => "",
        };
        if !key_str.is_empty() {
            match SecretKey::from_str(key_str) {
                Ok(sk) => signing_keys.push(sk),
                Err(e) => {
                    set_last_error(&format!("Invalid P2PK secret key hex: {e}"));
                    return 3;
                }
            }
        }
    }

    let h = unsafe { &*handle };
    let res: Result<u64, String> = h.rt.block_on(async {
        let recv_opts = ReceiveOptions {
            p2pk_signing_keys: signing_keys,
            ..Default::default()
        };
        let received_amount = h
            .wallet
            .receive(token, recv_opts)
            .await
            .map_err(|e| e.to_string())?;
        Ok(u64::from(received_amount))
    });

    match res {
        Ok(amount) => {
            unsafe {
                *out_received_sats = amount;
            }
            0
        }
        Err(e) => {
            set_last_error(&format!("CDK receive failed: {e}"));
            4
        }
    }
}

#[no_mangle]
pub extern "C" fn hanbova_cdk_check_token_state(
    handle: *mut CdkWalletHandle,
    token_str: *const c_char,
    out_state: *mut c_int, // 0 = Unspent, 1 = Pending, 2 = Spent, -1 = Unknown
) -> c_int {
    if handle.is_null() || token_str.is_null() || out_state.is_null() {
        set_last_error("Null pointer provided to hanbova_cdk_check_token_state");
        return 1;
    }

    let token = match unsafe { CStr::from_ptr(token_str) }.to_str() {
        Ok(s) => s,
        Err(e) => {
            set_last_error(&format!("Invalid token UTF-8: {e}"));
            return 2;
        }
    };

    let h = unsafe { &*handle };
    let res: Result<i32, String> = h.rt.block_on(async {
        // Parse token and check proof states via NUT-07
        let parsed_token = cdk::nuts::Token::from_str(token).map_err(|e| e.to_string())?;
        let keysets = h
            .wallet
            .localstore
            .get_mint_keysets(h.wallet.mint_url.clone())
            .await
            .map_err(|e| e.to_string())?
            .unwrap_or_default();
        let proofs = parsed_token.proofs(&keysets).map_err(|e| e.to_string())?;
        if proofs.is_empty() {
            return Ok(-1);
        }
        let states = h
            .wallet
            .check_proofs_spent(proofs)
            .await
            .map_err(|e| e.to_string())?;

        let mut has_unspent = false;
        let mut has_pending = false;
        let mut has_spent = false;

        for s in states {
            match s.state {
                cdk::nuts::State::Unspent => has_unspent = true,
                cdk::nuts::State::Pending => has_pending = true,
                cdk::nuts::State::Spent => has_spent = true,
                _ => {}
            }
        }

        if has_unspent {
            Ok(0) // Unspent
        } else if has_pending {
            Ok(1) // Pending
        } else if has_spent {
            Ok(2) // Spent
        } else {
            Ok(-1) // Unknown
        }
    });

    match res {
        Ok(st) => {
            unsafe {
                *out_state = st;
            }
            0
        }
        Err(e) => {
            set_last_error(&format!("Failed to check token state: {e}"));
            3
        }
    }
}

#[no_mangle]
pub extern "C" fn hanbova_cdk_wallet_free(handle: *mut CdkWalletHandle) {
    if !handle.is_null() {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
            let boxed = Box::from_raw(handle);
            boxed.rt.shutdown_background();
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ptr;

    #[test]
    fn test_cdk_ffi_lifecycle() {
        let mint_url = CString::new("http://127.0.0.1:3338").unwrap();
        let test_db_path =
            std::env::temp_dir().join(format!("hanbova_ffi_{}", uuid::Uuid::new_v4()));
        let db_path = CString::new(test_db_path.to_str().unwrap()).unwrap();
        let seed_hex = CString::new(
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        )
        .unwrap();

        let mut handle: *mut CdkWalletHandle = ptr::null_mut();
        let rc = hanbova_cdk_wallet_create(
            mint_url.as_ptr(),
            db_path.as_ptr(),
            seed_hex.as_ptr(),
            &mut handle,
        );
        assert_eq!(rc, 0);
        assert!(!handle.is_null());

        let mut spendable: u64 = 0;
        let mut pending: u64 = 0;
        let rc_bal = hanbova_cdk_wallet_get_balance(handle, &mut spendable, &mut pending);
        assert_eq!(rc_bal, 0);
        assert_eq!(spendable, 0);

        hanbova_cdk_wallet_free(handle);
    }
}
