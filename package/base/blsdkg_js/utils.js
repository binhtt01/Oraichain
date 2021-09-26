const hkdf = require('futoin-hkdf');
const { createCipheriv, createDecipheriv, randomBytes } = require('crypto');
const secp256k1 = require('secp256k1');
const Cosmos = require('@oraichain/cosmosjs').default;

const message = Cosmos.message;

const AES_IV_LENGTH = 16;
const AES_TAG_LENGTH = 16;
const AES_IV_PLUS_TAG_LENGTH = AES_IV_LENGTH + AES_TAG_LENGTH;

const multiply = (pub, priv) => {
  const ret = Buffer.from(secp256k1.publicKeyTweakMul(pub, priv, false));
  return ret;
};

// create a unique share key for each verification vector, to prevent leak of share key
const encapsulate = (priv, pub, commit) => {
  const master = Buffer.concat([commit, multiply(pub, priv)]);
  return hkdf(master, 32, {
    hash: 'SHA-256'
  });
};

const aesEncrypt = (key, plainText) => {
  const nonce = randomBytes(AES_IV_LENGTH);
  const cipher = createCipheriv('aes-256-gcm', key, nonce);
  const encrypted = Buffer.concat([cipher.update(plainText), cipher.final()]);
  const tag = cipher.getAuthTag();
  return Buffer.concat([nonce, tag, encrypted]);
};

const aesDecrypt = (key, cipherText) => {
  const nonce = cipherText.slice(0, AES_IV_LENGTH);
  const tag = cipherText.slice(AES_IV_LENGTH, AES_IV_PLUS_TAG_LENGTH);
  const ciphered = cipherText.slice(AES_IV_PLUS_TAG_LENGTH);
  const decipher = createDecipheriv('aes-256-gcm', key, nonce);
  decipher.setAuthTag(tag);
  return Buffer.concat([decipher.update(ciphered), decipher.final()]);
};

exports.encrypt = (pub, priv, commit, msg) => {
  const aesKey = encapsulate(priv, pub, commit);
  return aesEncrypt(aesKey, msg);
};

exports.decrypt = (priv, pub, commit, encrypted) => {
  const aesKey = encapsulate(priv, pub, commit);
  return aesDecrypt(aesKey, encrypted);
};

exports.queryWasm = async (cosmos, contract, input) => {
  const url = `/wasm/v1beta1/contract/${contract}/smart/${Buffer.from(
    JSON.stringify(input)
  ).toString('base64')}`;
  console.log(`${cosmos.url}${url}`);
  const { data } = await cosmos.get(url);
  return data;
};

const submit = async (cosmos, childKey, type, obj, { memo, fees, gas }) => {
  const paths = type.split('.');
  let childMessage = message;
  for (let p of paths) childMessage = childMessage[p];

  const msgSend = new childMessage(obj);

  const msgSendAny = new message.google.protobuf.Any({
    type_url: `/${type}`,
    value: childMessage.encode(msgSend).finish()
  });

  const txBody = new message.cosmos.tx.v1beta1.TxBody({
    messages: [msgSendAny],
    memo
  });

  try {
    const response = await cosmos.submit(
      childKey,
      txBody,
      'BROADCAST_MODE_BLOCK',
      isNaN(fees) ? 0 : parseInt(fees),
      isNaN(gas) ? 200000 : gas
    );
    return response;
  } catch (ex) {
    console.log(ex);
  }
};

exports.executeWasm = async (
  cosmos,
  childKey,
  contract,
  input,
  options = {}
) => {
  const msg = {
    contract,
    msg: Buffer.from(JSON.stringify(input)),
    sender: cosmos.getAddress(childKey)
  };
  const data = await submit(
    cosmos,
    childKey,
    'cosmwasm.wasm.v1beta1.MsgExecuteContract',
    msg,
    options
  );
  return data;
};