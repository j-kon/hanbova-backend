#ifndef HANBOVA_CDK_FFI_H
#define HANBOVA_CDK_FFI_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct CdkWalletHandle CdkWalletHandle;

char* hanbova_cdk_get_last_error(void);
void hanbova_cdk_free_string(char* s);

int hanbova_cdk_wallet_create(
    const char* mint_url,
    const char* db_path,
    const char* seed_hex,
    CdkWalletHandle** out_handle
);

int hanbova_cdk_wallet_get_balance(
    CdkWalletHandle* handle,
    uint64_t* out_spendable,
    uint64_t* out_pending
);

int hanbova_cdk_mint_quote(
    CdkWalletHandle* handle,
    uint64_t amount,
    char** out_quote_id,
    char** out_invoice
);

int hanbova_cdk_mint(
    CdkWalletHandle* handle,
    const char* quote_id,
    uint64_t* out_minted_amount
);

int hanbova_cdk_prepare_p2pk_send(
    CdkWalletHandle* handle,
    uint64_t amount,
    const char* recipient_pubkey_hex,
    const char* refund_pubkey_hex,
    uint64_t locktime_unix,
    char** out_token
);

int hanbova_cdk_receive_p2pk(
    CdkWalletHandle* handle,
    const char* token,
    const char* p2pk_privkey_hex,
    uint64_t* out_received_amount
);

int hanbova_cdk_check_token_state(
    CdkWalletHandle* handle,
    const char* token,
    int* out_is_spent
);

void hanbova_cdk_wallet_free(CdkWalletHandle* handle);

#ifdef __cplusplus
}
#endif

#endif /* HANBOVA_CDK_FFI_H */
