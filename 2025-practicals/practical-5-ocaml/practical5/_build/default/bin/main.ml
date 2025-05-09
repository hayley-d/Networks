(* open Unix;; *)
type org_unit = Friends | Acquaintances | Enemies

type entry = {
  cn : string;
  number : string;
  ou : org_unit;
}

let directory : entry list = [
  (* Friends *)
  { cn = "Gary Goodspeed"; number = "1110000001"; ou = Friends };
  { cn = "Mooncake"; number = "1110000002"; ou = Friends };
  { cn = "Quinn Ergon"; number = "1110000003"; ou = Friends };
  { cn = "Avocato"; number = "1110000004"; ou = Friends };
  { cn = "Little Cato"; number = "1110000005"; ou = Friends };

  (* Acquaintances *)
  { cn = "H.U.E."; number = "2220000001"; ou = Acquaintances };
  { cn = "KVN"; number = "2220000002"; ou = Acquaintances };
  { cn = "Tribore"; number = "2220000003"; ou = Acquaintances };
  { cn = "Fox"; number = "2220000004"; ou = Acquaintances };
  { cn = "Ash Graven"; number = "2220000005"; ou = Acquaintances };

  (* Enemies *)
  { cn = "The Lord Commander"; number = "3330000001"; ou = Enemies };
  { cn = "Invictus"; number = "3330000002"; ou = Enemies };
  { cn = "Clarence"; number = "3330000003"; ou = Enemies };
  { cn = "Todd Watson"; number = "3330000004"; ou = Enemies };
  { cn = "Nightfall"; number = "3330000005"; ou = Enemies }; 
]



(* BER utility functions *)
let read_byte sock = 
    let buf = Bytes.create 1 in
    ignore (Unix.read sock buf 0 1);
    int_of_char (Bytes.get buf 0)

let read_length sock =
  let len = read_byte sock in
  if len land 0x80 = 0 then len
  else
    let num_bytes = len land 0x7F in
    let rec read_n acc = function
      | 0 -> acc
      | n -> read_n ((acc lsl 8) lor (read_byte sock)) (n - 1)
    in
    read_n 0 num_bytes


let read_bytes sock len =
    let buf = Bytes.create len in
    ignore (Unix.read sock buf 0 len);
    buf

let make_response message_id cn number ou =
  (* Encode a BER Octet String (tag 0x04) *)
  let encode_str s =
    let len = String.length s in
    Char.chr 0x04 ::                 (* OCTET STRING tag *)
    (if len < 128 then [Char.chr len] else failwith "Too long") @
    List.init len (fun i -> s.[i])
  in

  (* Encode "cn" -> <cn value> *)
  let encoded_cn =
    let attr_type = encode_str "cn" in
    let attr_value = encode_str cn in
    [Char.chr 0x30;  (* SEQUENCE for one attribute *)
     Char.chr (List.length attr_type + List.length attr_value)
    ] @ attr_type @ attr_value
  in

  (* Encode "number" -> <number value> *)
  let encoded_tel =
    let attr_type = encode_str "number" in
    let attr_value = encode_str number in
    [Char.chr 0x30;
     Char.chr (List.length attr_type + List.length attr_value)
    ] @ attr_type @ attr_value
  in

  (* Wrap both attributes in an outer SEQUENCE *)
  let attrs =
    let inner = encoded_cn @ encoded_tel in
    [Char.chr 0x30; Char.chr (List.length inner)] @ inner
  in

  let ou_str = match ou with
    | Friends -> "Friends"
    | Acquaintances -> "Acquaintances"
    | Enemies -> "Enemies"
  in

  (* Construct the full DN string *)
  let dn_string = "cn=" ^ cn ^ ",ou=" ^ ou_str ^ ",dc=example,dc=com" in
  let encoded_dn = encode_str dn_string in

  (* Construct the SearchResultEntry (APPLICATION 4 / tag 0x64) *)
  let entry_content = encoded_dn @ attrs in
  let entry =
    [Char.chr 0x64; Char.chr (List.length entry_content)] @ entry_content
  in

  (* Encode the message ID as INTEGER (tag 0x02) *)
  let msg_id = [Char.chr 0x02; Char.chr 0x01; Char.chr message_id] in

  (* Wrap everything in an outer LDAPMessage SEQUENCE (tag 0x30) *)
  let full = msg_id @ entry in
  let full_len = List.length full in
  let ldap_message = [Char.chr 0x30; Char.chr full_len] @ full in

  (* Convert char list to bytes *)
  Bytes.of_string (String.init (List.length ldap_message) (fun i -> List.nth ldap_message i))


(* LDAP client handling *)
let handle_client sock =
  Printf.printf "Client connected\n%!";
  let tag = read_byte sock in
  if tag <> 0x30 then failwith "Expected SEQUENCE";

  let _len = read_length sock in
  let _msg_id_tag = read_byte sock in
  let _msg_id_len = read_length sock in
  let msg_id = read_byte sock in

  let op_tag = read_byte sock in
  if op_tag = 0x60 then
    let _ = read_length sock in
    ignore (read_bytes sock 3); 
    ()
  else if op_tag = 0x63 then
    let _ = read_length sock in
    let base_dn_tag = read_byte sock in
    Printf.printf "Base DN tag: 0x%02X\n" base_dn_tag;
    let base_dn_len = read_length sock in
    let base_dn_bytes = read_bytes sock base_dn_len in
    let base_dn = Bytes.to_string base_dn_bytes in
    Printf.printf "Client queried for: %s\n%!" base_dn;

    let cn =
      try
        Scanf.sscanf base_dn "cn=%s@," (fun x -> x)
      with _ -> ""
    in

    let found =
        List.find_opt
            (fun e -> e.ou = Friends && String.lowercase_ascii e.cn = String.lowercase_ascii cn)
        directory
    in

    begin match found with
    | Some e ->
        let response = make_response msg_id e.cn e.number e.ou in
        ignore (Unix.write sock response 0 (Bytes.length response))
    | None ->
        Printf.printf "Friend not found: %s\n" base_dn
    end
  else
    Printf.printf "Unknown operation tag: %d\n" op_tag

(* Start a TCP server *)
let start_server () =
    (* Create a socket using the port number *)
    let sock = Unix.socket Unix.PF_INET Unix.SOCK_STREAM 0 in
    Unix.setsockopt sock Unix.SO_REUSEADDR true;
    Unix.bind sock (Unix.ADDR_INET (Unix.inet_addr_any, 7878));
    Unix.listen sock 5;
    Printf.printf "LDAP server running on port 7878\n%!";
    while true do
        let client, _ = Unix.accept sock in
        match Unix.fork () with
        | 0 -> Unix.close sock;
            handle_client client;
            Unix.close client;
            exit 0
        | _ -> Unix.close client
    done

(* Entry point *)
let () = 
    Printf.printf "Starting server....\n%!";
    start_server ()


