



use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    program_error::ProgramError,
    msg,
    pubkey::Pubkey,
    program_pack::{Pack, IsInitialized},
    sysvar::{rent::Rent, Sysvar},
    program::invoke
};

use crate::{instruction::EscrowInstruction, error::EscrowError, state::Escrow};


pub struct Processor;
impl Processor {
    pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], instruction_data: &[u8]) -> ProgramResult {
        let instruction = EscrowInstruction::unpack(instruction_data)?;

        match instruction {
            EscrowInstruction::InitEscrow { amount } => {
                msg!("Instruction: InitEscrow");
                Self::process_init_escrow(accounts, amount, program_id)
            },
            EscrowInstruction::Exchange { amount } => {
                msg!("Instruction: Exchange");
                Self::process_exchange(accounts, amount, program_id)
            }
        }
    }

    fn process_init_escrow(
        accounts: &[AccountInfo],
        amount: u64,
        program_id: &Pubkey,
    ) -> ProgramResult {

        /// Accounts
        /// 0: Signer
        /// 1: temp_token
        /// 2: receive token - owner must be spl token id
        /// 3: escrow
        /// 4: rent
        /// 5: token program

        let account_info_iter = &mut accounts.iter();
        let initializer = next_account_info(account_info_iter)?; // account

        if !initializer.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }


        let temp_token_account = next_account_info(account_info_iter)?; // account

        let token_to_receive_account = next_account_info(account_info_iter)?; // account
        if *token_to_receive_account.owner != spl_token::id() {
            return Err(ProgramError::IncorrectProgramId);
        }

        let escrow_account = next_account_info(account_info_iter)?; // account
        let rent = &Rent::from_account_info(next_account_info(account_info_iter)?)?; // account
        
        if !rent.is_exempt(escrow_account.lamports(), escrow_account.data_len()) {
            return Err(EscrowError::NotRentExempt.into());
        }
        
        // prechecks done

        // Fill escrow
        let mut escrow_info = Escrow::unpack_unchecked(&escrow_account.try_borrow_data()?)?;
        if escrow_info.is_initialized() {
            return Err(ProgramError::AccountAlreadyInitialized);
        }

        escrow_info.is_initialized = true;
        escrow_info.initializer_pubkey = *initializer.key;
        escrow_info.temp_token_account_pubkey = *temp_token_account.key;
        escrow_info.initializer_token_to_receive_account_pubkey = *token_to_receive_account.key;
        escrow_info.expected_amount = amount;

        Escrow::pack(escrow_info, &mut escrow_account.try_borrow_mut_data()?)?;

        /// what means this b"escrow"?
        /// pda - program derived address
        let (pda, _bump_seed) = Pubkey::find_program_address(&[b"escrow"], program_id);

        let token_program = next_account_info(account_info_iter)?; // account
        let owner_change_ix = spl_token::instruction::set_authority(
            token_program.key,
            temp_token_account.key,
            Some(&pda),
            spl_token::instruction::AuthorityType::AccountOwner,
            initializer.key,
            &[&initializer.key],
        )?;

        msg!("Calling the token program to transfer token account ownership...");
        invoke(
            &owner_change_ix,
            &[
                temp_token_account.clone(),
                initializer.clone(),
                token_program.clone(),
            ],
        )?;

        Ok(())
    }

    fn process_exchange(
        accounts: &[AccountInfo],
        amount_expected_by_taker: u64,
        program_id: &Pubkey,
    ) -> ProgramResult {

        /// Accounts
        /// 0: Signer
        /// 1: sending token
        /// 2: to receive token - owner must be spl token id
        /// 3: temp token
        /// 4: initializers main account
        /// 5: initializers token to receive account
        /// 6: escrow
        /// 7: token program

        let account_info_iter = &mut accounts.iter();
        let taker = next_account_info(account_info_iter)?; // account
    
        if !taker.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }
    
        let takers_sending_token_account = next_account_info(account_info_iter)?; // account
    
        let takers_token_to_receive_account = next_account_info(account_info_iter)?; // account
    
        let pdas_temp_token_account = next_account_info(account_info_iter)?; // account
        let pdas_temp_token_account_info =
            TokenAccount::unpack(&pdas_temp_token_account.try_borrow_data()?)?;
        // here it is again b"escrow"
        /// pda - program derived address
        let (pda, bump_seed) = Pubkey::find_program_address(&[b"escrow"], program_id);
    
        if amount_expected_by_taker != pdas_temp_token_account_info.amount {
            return Err(EscrowError::ExpectedAmountMismatch.into());
        }
    
        let initializers_main_account = next_account_info(account_info_iter)?; // account
        let initializers_token_to_receive_account = next_account_info(account_info_iter)?; // account
        let escrow_account = next_account_info(account_info_iter)?; // account
    
        let escrow_info = Escrow::unpack(&escrow_account.try_borrow_data()?)?;
    
        if escrow_info.temp_token_account_pubkey != *pdas_temp_token_account.key {
            return Err(ProgramError::InvalidAccountData);
        }
    
        if escrow_info.initializer_pubkey != *initializers_main_account.key {
            return Err(ProgramError::InvalidAccountData);
        }
    
        if escrow_info.initializer_token_to_receive_account_pubkey != *initializers_token_to_receive_account.key {
            return Err(ProgramError::InvalidAccountData);
        }

        // prechecks done
    
        let token_program = next_account_info(account_info_iter)?; // account
    
        let transfer_to_initializer_ix = spl_token::instruction::transfer(
            token_program.key,
            takers_sending_token_account.key,
            initializers_token_to_receive_account.key,
            taker.key,
            &[&taker.key],
            escrow_info.expected_amount,
        )?;
        msg!("Calling the token program to transfer tokens to the escrow's initializer...");
        invoke(
            &transfer_to_initializer_ix,
            &[
                takers_sending_token_account.clone(),
                initializers_token_to_receive_account.clone(),
                taker.clone(),
                token_program.clone(),
            ],
        )?;


        let pda_account = next_account_info(account_info_iter)?; // account

        let transfer_to_taker_ix = spl_token::instruction::transfer(
            token_program.key,
            pdas_temp_token_account.key,
            takers_token_to_receive_account.key,
            &pda,
            &[&pda],
            pdas_temp_token_account_info.amount,
        )?;
        msg!("Calling the token program to transfer tokens to the taker...");
        invoke_signed(
            &transfer_to_taker_ix,
            &[
                pdas_temp_token_account.clone(),
                takers_token_to_receive_account.clone(),
                pda_account.clone(),
                token_program.clone(),
            ],
            &[&[&b"escrow"[..], &[bump_seed]]],
        )?;
        
        let close_pdas_temp_acc_ix = spl_token::instruction::close_account(
            token_program.key,
            pdas_temp_token_account.key,
            initializers_main_account.key,
            &pda,
            &[&pda]
        )?;
        msg!("Calling the token program to close pda's temp account...");
        invoke_signed(
            &close_pdas_temp_acc_ix,
            &[
                pdas_temp_token_account.clone(),
                initializers_main_account.clone(),
                pda_account.clone(),
                token_program.clone(),
            ],
            &[&[&b"escrow"[..], &[bump_seed]]],
        )?;

        msg!("Closing the escrow account...");
        **initializers_main_account.lamports.borrow_mut() = initializers_main_account
            .lamports()
            .checked_add(escrow_account.lamports())
            .ok_or(EscrowError::AmountOverflow)?;
        **escrow_account.lamports.borrow_mut() = 0;
        *escrow_account.try_borrow_mut_data()? = &mut [];


        Ok(())
    }
}
